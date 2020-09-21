#![allow(unused_must_use)]

#[macro_use]
extern crate lazy_static;

use std::io;
use std::path::Path;

use actix_web::{App, get, HttpResponse, HttpServer, web};
use serde_json::json;

use crate::media::Sessions;
use crate::settings::Settings;

mod commands;
mod settings;
mod media;
mod dash;

lazy_static! {
    static ref SETTINGS: Settings = Settings::new().unwrap();
    static ref UNPROCESSED_DIR: &'static Path = Path::new(&(*SETTINGS).dirs.unprocessed);
    static ref PROCESSED_DIR: &'static Path = Path::new(&(*SETTINGS).dirs.processed);
}

#[get("/")]
async fn index() -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok().json(json!({
    "item": "Hello, World!"
    })))
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init();
    std::fs::read_dir(*UNPROCESSED_DIR).expect("unprocessed dirs");
    std::fs::read_dir(*PROCESSED_DIR).expect("processed dirs");

    let state = web::Data::new(Sessions::new());

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(media::unprocessed)
            .service(media::processed)
            .service(media::process)
            .service(media::get_session)
            .service(media::all_sessions)
            .service(index)
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
