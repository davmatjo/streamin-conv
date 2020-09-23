use core::result::Result::{Err, Ok};
use std::error::Error;
use std::path::PathBuf;

use tokio::process::Command;

use crate::commands::{MediaCommandConfig, SessionError};
use crate::commands::ffmpeg::Encoder::{Audio, Subtitle, Video};
use crate::commands::SessionError::InvalidCommandConfig;

pub struct Config {
    video: CodecOpts,
    audio: CodecOpts,
    subtitle: CodecOpts,
    file: PathBuf,
    out_file: Option<PathBuf>,
    tracks: Vec<isize>,
    can_fail: bool,
}

pub struct CodecOpts {
    encoder: Encoder,
    bitrate: isize,
    enabled: bool,
    crf: isize,
    channels: isize,
    colour_8_bit: bool,
}

#[derive(PartialEq)]
pub enum Encoder {
    Video(VideoEncoder),
    Audio(AudioEncoder),
    Subtitle(SubtitleEncoder),
    None,
}

type VideoEncoder = &'static str;

pub const X264: VideoEncoder = "libx264";
#[allow(dead_code)]
pub const X265: VideoEncoder = "libx264";
#[allow(dead_code)]
pub const X264_NVENC: VideoEncoder = "libx264";
#[allow(dead_code)]
pub const X265_NVENC: VideoEncoder = "libx264";


type AudioEncoder = &'static str;

pub const AAC: AudioEncoder = "aac";


type SubtitleEncoder = &'static str;

pub const WEB_VTT: SubtitleEncoder = "webvtt";

impl MediaCommandConfig for Config {
    fn build(&self) -> Result<Command, Box<dyn Error>> {
        self.validate()?;

        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-i")
            .arg(&self.file)
            .arg("-y")
            // .arg("-v")
            // .arg("quiet")
            .arg("-progress")
            .arg("-");

        if self.video.enabled {
            let enc = match self.video.encoder {
                Video(x) => x,
                Encoder::None => "copy",
                _ => unreachable!()
            };

            cmd.arg("-c:v")
                .arg(enc);

            if self.video.bitrate > -1 {
                cmd.arg("-b:v")
                    .arg(self.video.bitrate.to_string());
            }

            if self.video.colour_8_bit {
                cmd.arg("-vf")
                    .arg("format=yuv420p");
            }

            if self.video.crf > -1 {
                cmd.arg("-crf")
                    .arg(self.video.crf.to_string());
            }
        } else {
            cmd.arg("-vn");
        }

        if self.audio.enabled {
            let enc = match self.audio.encoder {
                Audio(x) => x,
                Encoder::None => "copy",
                _ => unreachable!()
            };

            cmd.arg("-c:a")
                .arg(enc);

            if self.audio.bitrate > -1 {
                cmd.arg("-b:a")
                    .arg(self.audio.bitrate.to_string());
            }

            if self.audio.channels > -1 {
                cmd.arg("-ac")
                    .arg(self.audio.channels.to_string());
            }
        } else {
            cmd.arg("-an");
        }

        if self.subtitle.enabled {
            let enc = match self.subtitle.encoder {
                Subtitle(x) => x,
                Encoder::None => "copy",
                _ => unreachable!()
            };

            cmd.arg("-c:s")
                .arg(enc);
        } else {
            cmd.arg("-sn");
        }

        for t in &self.tracks {
            cmd.arg("-map")
                .arg("0:".to_string() + &*t.to_string());
        }

        let out = self.out_file.clone().unwrap_or({
            let mut base = std::env::temp_dir();
            let mut stem = self.file.file_stem().unwrap().to_os_string();
            stem.push({
                let idx = self.tracks.get(0).cloned().unwrap_or(0);
                if self.video.enabled {
                    format!("-split-vid-{}.mp4", idx)
                } else if self.audio.enabled {
                    format!("-split-aud-{}.mp4", idx)
                } else {
                    format!("-split-sub-{}.vtt", idx)
                }
            });
            base.push(stem);
            base
        });
        cmd.arg(&out);

        Ok(cmd)
    }

    fn validate(&self) -> Result<(), SessionError> {
        match self.video.encoder {
            Audio(_) | Subtitle(_) => Err(InvalidCommandConfig("video cannot have an audio or subtitle encoder")),
            Video(_) | Encoder::None => Ok(())
        }?;
        match self.audio.encoder {
            Video(_) | Subtitle(_) => Err(InvalidCommandConfig("audio cannot have an video or subtitle encoder")),
            Audio(_) | Encoder::None => Ok(())
        }?;
        match self.subtitle.encoder {
            Audio(_) | Video(_) => Err(InvalidCommandConfig("subtitle cannot have an audio or video encoder")),
            Subtitle(_) | Encoder::None => Ok(())
        }?;

        if !self.video.enabled && !self.audio.enabled && !self.subtitle.enabled {
            return Err(InvalidCommandConfig("no streams are enabled"));
        }

        if self.audio.crf > -1 || self.subtitle.crf > -1 {
            return Err(InvalidCommandConfig("audio and subtitles cannot have a crf"));
        }

        if (self.video.bitrate > -1 || self.video.crf > -1) && self.video.encoder == Encoder::None {
            return Err(InvalidCommandConfig("bitrate and crf cannot be set without an encoder"));
        }

        Ok(())
    }

    fn can_fail(&self) -> bool {
        self.can_fail
    }
}

#[allow(dead_code)]
impl Config {
    pub fn new(file: PathBuf) -> Self {
        Config {
            file,
            out_file: None,
            tracks: vec![],
            video: CodecOpts {
                encoder: Encoder::None,
                bitrate: -1,
                enabled: true,
                crf: -1,
                channels: -1,
                colour_8_bit: false,
            },
            audio: CodecOpts {
                encoder: Encoder::None,
                bitrate: -1,
                enabled: true,
                crf: -1,
                channels: -1,
                colour_8_bit: false,
            },
            subtitle: CodecOpts {
                encoder: Encoder::None,
                bitrate: -1,
                enabled: true,
                crf: -1,
                channels: -1,
                colour_8_bit: false,
            },
            can_fail: false,
        }
    }

    pub fn out(&mut self, out: PathBuf) -> &mut Self {
        self.out_file = Some(out);
        self
    }

    pub fn crf(&mut self, crf: isize) -> &mut Self {
        self.video.crf = crf;
        self
    }

    pub fn video_bitrate(&mut self, b: isize) -> &mut Self {
        self.video.bitrate = b;
        self
    }

    pub fn audio_bitrate(&mut self, b: isize) -> &mut Self {
        self.audio.bitrate = b;
        self
    }

    pub fn video_encoder(&mut self, e: VideoEncoder) -> &mut Self {
        self.video.encoder = Video(e);
        self
    }

    pub fn audio_encoder(&mut self, e: AudioEncoder) -> &mut Self {
        self.audio.encoder = Audio(e);
        self
    }

    pub fn subtitle_encoder(&mut self, e: SubtitleEncoder) -> &mut Self {
        self.subtitle.encoder = Subtitle(e);
        self
    }

    pub fn video_disabled(&mut self) -> &mut Self {
        self.video.enabled = false;
        self
    }

    pub fn audio_disabled(&mut self) -> &mut Self {
        self.audio.enabled = false;
        self
    }

    pub fn subtitle_disabled(&mut self) -> &mut Self {
        self.subtitle.enabled = false;
        self
    }

    pub fn audio_channels(&mut self, channels: isize) -> &mut Self {
        self.audio.channels = channels;
        self
    }

    pub fn tracks<T>(&mut self, tracks: T) -> &mut Self
        where
            T: IntoIterator<Item=isize>,
    {
        self.tracks.extend(tracks);
        self
    }

    pub fn colour_8_bit(&mut self) -> &mut Self {
        self.video.colour_8_bit = true;
        self
    }

    pub fn can_fail(&mut self) -> &mut Self {
        self.can_fail = true;
        self
    }
}

