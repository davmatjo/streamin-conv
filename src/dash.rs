use std::iter::once;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use actix_web::web::Data;
use uuid::Uuid;

use crate::commands::{ffmpeg, MediaInfo, mp4dash, mp4fragment, Session};
use crate::commands::ffmpeg::{AAC, WEB_VTT, X264};
use crate::media::Sessions;

// The 'business logic' of the main functionality of the API, this method will convert a given video
// file into a directory containing a dash manifest and all segments. This is achieved by chaining
// various Configs together into a Session. The session enables reporting of status through some
// shared memory, and coordinates the list of commands to execute.
pub(crate) fn exec_dash_conv(state: Data<Sessions>, file: PathBuf) -> String {
    let id = Uuid::new_v4();
    let info = MediaInfo::get(&file).unwrap();

    let mut vid = ffmpeg::Config::new(file.clone());
    if info.dash_transcode_required() {
        vid.video_encoder(X264)
            .crf(19)
            .colour_8_bit();
    }
    vid.audio_disabled()
        .subtitle_disabled();

    let audios: Vec<_> = info.raw.streams.iter().filter(|s| s.codec_type == "audio").map(|s| {
        let mut aud = ffmpeg::Config::new(file.clone());
        aud.video_disabled()
            .subtitle_disabled()
            .audio_channels(2)
            .audio_encoder(AAC)
            .audio_bitrate(256_000)
            .tracks(once(s.index));
        aud
    }).collect();

    let subs: Vec<_> = info.raw.streams.iter().filter(|s| s.codec_type == "subtitle").map(|s| {
        let mut sub = ffmpeg::Config::new(file.clone());
        sub.video_disabled()
            .audio_disabled()
            .subtitle_encoder(WEB_VTT)
            .tracks(once(s.index));
        sub
    }).collect();

    let vid_frag = mp4fragment::Config::new(temp_new_file_end(file.as_path(), "-split-vid-0.mp4"));
    let audio_frags: Vec<_> = info.raw.streams.iter().filter(|s| s.codec_type == "audio").map(|s| {
        mp4fragment::Config::new(temp_new_file_end(file.as_path(), &*format!("-split-aud-{}.mp4", s.index)))
    }).collect();

    let dash = mp4dash::Config::new(
        info.raw.streams.iter().filter_map(|s| {
            match &*s.codec_type {
                "video" if s.index == 0 => Some(temp_new_file_end(file.as_path(), &*format!("-split-vid-{}-f.mp4", s.index))),
                "audio" => Some(temp_new_file_end(file.as_path(), &*format!("-split-aud-{}-f.mp4", s.index))),
                "subtitle" => Some(temp_new_file_end(file.as_path(), &*format!("-split-sub-{}.vtt", s.index))),
                _ => None
            }
        })
    );

    let info = Arc::new(RwLock::new(info));
    let mut session = Session::new(Box::new(vid), info);
    for a in audios {
        session.chain(a);
    }
    for s in subs {
        session.chain(s);
    }
    session.chain(vid_frag);
    for a in audio_frags {
        session.chain(a);
    }
    session.chain(dash);
    session.start().unwrap();

    state.sessions.write().unwrap().insert(id, session);
    id.to_string()
}

fn temp_new_file_end(file: &Path, ending: &str) -> PathBuf {
    let mut temp = std::env::temp_dir();
    let mut stem = file.file_stem().unwrap().to_os_string();
    stem.push(ending);
    temp.push(stem);
    temp
}
