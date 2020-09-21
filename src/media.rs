use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::RwLock;

use actix_web::{get, HttpResponse, post};
use actix_web::web;
use actix_web::web::Data;
use derive_more::{Display, Error};
use log::error;
use rayon::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{commands, dash, PROCESSED_DIR, UNPROCESSED_DIR};
use crate::commands::{MediaInfo, Session};
use crate::media::UserError::{NotFound};

pub struct Sessions {
    pub(crate) sessions: RwLock<HashMap<Uuid, Session>>
}

impl Sessions {
    pub fn new() -> Self {
        Sessions {
            sessions: RwLock::new(HashMap::new())
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct ProcessReq {
    id: String,
    dash: Option<bool>,
}

#[derive(Debug, Display, Error)]
enum UserError {
    // #[display(fmt = "An internal error occurred. Please try again later.")]
    // Internal,
    #[display(fmt = "Not found")]
    NotFound,
}

fn log_not_found<T>(e: T) -> actix_web::Error
    where T: Error
{
    error!("{}", e);
    actix_web::error::ErrorNotFound(NotFound)
}

#[post("/media/process")]
pub async fn process(req: web::Json<ProcessReq>, state: Data<Sessions>) -> Result<HttpResponse, actix_web::Error> {
    // We return NotFoundError in most cases to avoid information leakage
    let res = base64::decode(&req.id)
        .map_err(log_not_found)?;

    let canonical = Path::new(std::str::from_utf8(&res)
        .map_err(log_not_found)?)
        .canonicalize().map_err(log_not_found)?;

    let dir = *UNPROCESSED_DIR;
    if canonical.starts_with(dir.canonicalize()?) && canonical.exists() {
        if let Some(true) = req.dash {
            return Ok(HttpResponse::Created().header("Location", dash::exec_dash_conv(state, canonical)).finish());
        };
    }

    Err(actix_web::error::ErrorNotFound(NotFound))
}

#[get("/media/process/session")]
pub async fn all_sessions(state: Data<Sessions>) -> Result<HttpResponse, actix_web::Error> {
    let sessions: HashMap<_, _> = state.sessions
        .read()
        .unwrap()
        .iter()
        .map(|s| (*s.0, s.1.get_info()))
        .collect();

    Ok(HttpResponse::Ok().json(sessions))
}

#[get("/media/process/session/{id}")]
pub async fn get_session(web::Path(id): web::Path<String>, state: Data<Sessions>) -> Result<HttpResponse, actix_web::Error> {
    println!("{}", id);
    let id = Uuid::parse_str(id.as_str()).map_err(log_not_found)?;
    println!("{}", id);

    let sessions = state.sessions.read().unwrap();
    let session = sessions.get(&id).ok_or_else(|| log_not_found(NotFound))?;
    Ok(HttpResponse::Ok().json(session.get_info()))
}

#[get("/media/unprocessed")]
pub async fn unprocessed() -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok().json(get_media_infos(*UNPROCESSED_DIR)))
}

#[get("/media/processed")]
pub async fn processed() -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok().json(get_media_infos(*PROCESSED_DIR)))
}

fn get_media_infos(dir: &Path) -> Vec<MediaInfo> {
    // Splits the files into a parallel iterator and runs ffprobe on each media file, ignoring any invalid files
    // This will not panic unless directories are deleted during execution
    std::fs::read_dir(dir).unwrap().par_bridge().filter_map(|entry| {
        entry.ok().and_then(|e| {
            commands::MediaInfo::get(e.path().as_path()).ok()
        })
    }).collect()
}