use std::collections::VecDeque;
use std::error::Error;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use derive_more::{Display, Error};
use log::error;
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::commands::ffprobe::FFProbeResponse;
use crate::commands::SessionError::AlreadyStarted;

mod ffprobe;
pub mod ffmpeg;
pub mod mp4fragment;
pub mod mp4dash;

#[derive(Display, Debug, Error)]
pub enum SessionError {
    #[display(fmt = "The session has already been started")]
    AlreadyStarted,
    #[display(fmt = "The command has ended up with an impossible configuration: {}", _0)]
    InvalidCommandConfig(#[error(not(source))] &'static str),
}

pub trait MediaCommandConfig {
    fn build(&self) -> Result<Command, Box<dyn Error>>;
    fn validate(&self) -> Result<(), SessionError>;
}

pub struct Session {
    media_info: Arc<RwLock<MediaInfo>>,
    session_info: Arc<RwLock<SessionInfoInt>>,
    commands: Vec<Box<dyn MediaCommandConfig + Send + Sync>>,
}

#[derive(Clone)]
pub struct SessionInfoInt {
    frame: usize,
    fps: f64,
    bitrate: f64,
    total_size: usize,
    time: Duration,
    stdout: Vec<String>,
    stderr: Vec<String>,
    stage: usize,
    max_stages: usize,
}

#[derive(Serialize, Debug)]
pub struct SessionInfo {
    percent_complete: f64,
    stage: usize,
    max_stages: usize,
    detail: Option<SessionDetail>,
    logs: SessionLog,
}

#[derive(Serialize, Debug)]
pub struct SessionLog {
    stdout: Vec<String>,
    stderr: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct SessionDetail {
    frame: usize,
    fps: f64,
    bitrate: f64,
    total_size: usize,
    time: Duration,
    length: Duration,
}

impl Session {
    pub fn new(cmd: Box<dyn MediaCommandConfig + Send + Sync>, info: Arc<RwLock<MediaInfo>>) -> Self
    {
        let session = Arc::new(RwLock::new(SessionInfoInt {
            frame: 0,
            fps: 0.0,
            bitrate: 0.0,
            total_size: 0,
            time: Duration::from_secs(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
            stage: 0,
            max_stages: 1,
        }));

        Session {
            media_info: info,
            session_info: session,
            commands: vec![cmd],
        }
    }

    pub fn get_info(&self) -> SessionInfo {
        let media_info = &*self.media_info.read().unwrap();
        let session_info = &*self.session_info.read().unwrap();

        let task_percent =
            session_info.time.as_secs() as f64 / media_info.duration.as_secs() as f64 * 100.0;

        let overall_percent =
            ((session_info.stage as f64 - 1.0) / session_info.max_stages as f64) * 100.0
                + (task_percent / session_info.max_stages as f64);

        let detail = if session_info.bitrate > 0.0 {
            Some(SessionDetail {
                frame: session_info.frame,
                fps: session_info.fps,
                bitrate: session_info.bitrate,
                total_size: session_info.total_size,
                time: session_info.time,
                length: media_info.duration,
            })
        } else {
            None
        };

        SessionInfo {
            percent_complete: overall_percent,
            stage: session_info.stage,
            max_stages: session_info.max_stages,

            logs: SessionLog {
                stdout: session_info.stdout.clone(),
                stderr: session_info.stderr.clone(),
            },
            detail,
        }
    }

    pub fn chain<T: 'static>(&mut self, cmd: T) -> &mut Self
        where T: MediaCommandConfig + Send + Sync
    {
        self.commands.push(Box::new(cmd));
        self
    }

    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        if self.commands.is_empty() {
            return Err(Box::new(AlreadyStarted));
        }
        self.session_info.write().unwrap().max_stages = self.commands.len();

        let cmds = std::mem::replace(&mut self.commands, vec![]);
        let cmds: Vec<Command> = cmds.iter().map(|c| c.build()).collect::<Result<_, _>>()?;

        let status = self.session_info.clone();
        let max_time = self.media_info.read().unwrap().duration.clone();

        tokio::spawn(async move {
            let status = status;
            for cmd in cmds {
                println!("Spawning cmd: {:?}", cmd);
                status.write().unwrap().stage += 1;
                Self::spawn(cmd, status.clone()).await;
            }
            // Manually max out the time to ensure we're at 100%
            status.write().unwrap().time = max_time;
        });
        Ok(())
    }

    async fn spawn(mut cmd: Command, status: Arc<RwLock<SessionInfoInt>>) {
        cmd.stdout(Stdio::piped())
            .stdin(Stdio::null())
            .stderr(Stdio::piped());
        println!("Starting cmd");

        let mut p = cmd.spawn().unwrap();

        let stdout = p.stdout.take().unwrap();
        let stderr = p.stderr.take().unwrap();

        let mut reader = BufReader::new(stdout).lines();
        let mut reader_err = BufReader::new(stderr).lines();

        let status_stdout = status.clone();
        tokio::spawn(async move {
            let mut local_buf = SessionInfoInt {
                frame: 0,
                fps: 0.0,
                bitrate: 0.0,
                total_size: 0,
                time: Default::default(),
                stdout: vec![],
                stderr: vec![],
                stage: 0,
                max_stages: 0,
            };
            let mut line_buf = VecDeque::new();
            let mut ctr = 0;

            {
                let s = &mut *status_stdout.write().unwrap();
                s.frame = 0;
                s.fps = 0.0;
                s.bitrate = 0.0;
                s.total_size = 0;
                s.time = Default::default();
            }

            while let Some(line) = reader.next_line().await.unwrap() {
                match line.split('=').collect::<Vec<_>>()[..] {
                    ["frame", x] => local_buf.frame = x.parse().unwrap_or(local_buf.frame),
                    ["fps", x] => local_buf.fps = x.parse().unwrap_or(local_buf.fps),
                    ["bitrate", x] => local_buf.bitrate = x[..x.len() - 7].trim().parse().unwrap_or(local_buf.bitrate),
                    ["total_size", x] => local_buf.total_size = x.trim().parse().unwrap_or(local_buf.total_size),
                    ["out_time_us", x] => local_buf.time = Duration::from_micros(x.parse().unwrap_or_else(|_| local_buf.time.as_micros() as u64)),
                    [_, _] => (),
                    _ => {
                        // Unknown line implies we want to know immediately
                        line_buf.push_back(line);
                        ctr = 25;
                    }
                }

                // Limit updates to limit locks
                if ctr > 24 {
                    let s = &mut *status_stdout.write().unwrap();
                    s.frame = local_buf.frame;
                    s.fps = local_buf.fps;
                    s.bitrate = local_buf.bitrate;
                    s.total_size = local_buf.total_size;
                    s.time = local_buf.time;

                    s.stdout.extend(line_buf.drain(..));

                    ctr = 0;
                }
                ctr += 1;
            };
        });

        tokio::spawn(async move {
            while let Some(line) = reader_err.next_line().await.unwrap() {
                println!("{}", line);
                let s = &mut *status.write().unwrap();
                s.stderr.push(line);
            };
        });

        // Ensure the child process is spawned in the runtime so it can
        // make progress on its own while we await for any output.
        tokio::spawn(async {
            let status = p.await
                .expect("child process encountered an error");
            println!("child status was: {}", status);
        }).await;
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct MediaInfo {
    pub id: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub meta_title: Option<String>,
    pub file_title: String,
    pub duration: Duration,

    #[serde(skip)]
    pub raw: FFProbeResponse,
}

impl MediaInfo {
    pub fn get(file: &Path) -> Result<Self, Box<dyn Error>> {
        let meta = ffprobe::get_info(&file)?;

        let v = meta.streams.iter().find(|s| s.codec_type == "video");
        let a = meta.streams.iter().find(|s| s.codec_type == "audio");

        Ok(
            MediaInfo {
                id: base64::encode_config(file.to_str().unwrap(), base64::URL_SAFE_NO_PAD),
                video_codec: v.and_then(|v| v.codec_name.clone().into()),
                audio_codec: a.and_then(|a| a.codec_name.clone().into()),
                meta_title: v.and_then(|v| v.tags.title.clone()),
                file_title: file.file_name().unwrap().to_str().unwrap().to_string(),
                duration: Duration::from_secs_f64(meta.format.duration.parse().unwrap()),
                raw: meta,
            }
        )
    }

    pub fn dash_transcode_required(&self) -> bool {
        match &self.video_codec {
            Some(x) => x != "h264",
            None => true
        }
    }
}
