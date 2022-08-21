mod config;
mod log;

use chrono;
use handlebars;
use std::fs::metadata;
use std::time::SystemTime;

use crate::config::CONFIG;
use actix_web::body::{BodySize, BodyStream};
use actix_web::error::HttpError;
use actix_web::http::header::DATE;
use actix_web::web::{Bytes, Header};
use actix_web::{
    delete, get, options, put, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use chrono::{Date, DateTime, Duration, FixedOffset, Local, NaiveDate, Utc};
use handlebars::Handlebars;
use iso8601::Date;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper;
use scraper::html::Select;
use scraper::node::Element;
use scraper::{Html, Selector};
use sled::Tree;

static KEY_VALIDATOR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"/83e(0[1-9]|1[0-2])(\d\d)$/").unwrap());

static TIME_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("time").unwrap());

static SIMPLE_ROOT_PAGE: Lazy<String> = Lazy::new(|| {
    let mut handlebars = Handlebars::new();
    let source = include_str!("./root_page.html");

    handlebars
        .register_template_string("title", CONFIG.simple_root_page.title)
        .register_template_string("header", CONFIG.simple_root_page.header)
        .unwrap();
});

static TREE: Lazy<Tree> = Lazy::new(|| {
    sled::open(&CONFIG.sled_dir).unwrap_or_else(|e| {
        log::e("main.rs::TREE::open", format!("{:?}", e), 3);
        panic!();
    })
});

#[get("/")]
async fn index() -> impl Responder {}

// TODO: verify signature
#[put("/{key}")]
async fn put(key: web::Path<String>, bytes: Bytes) -> impl Responder {
    // check board size first
    if bytes.len() > 2217 {
        Err(HttpResponse::PayloadTooLarge().into())
    }

    // check & verify key
    let key = key.into_inner();
    let matches = KEY_VALIDATOR.is_match(&key);
    // get 4 last digits of key (MMYY)
    let key_date = &key[key.len() - 4..];
    // parse as expiry date (MMYY)
    let expiry_date = DateTime::parse_from_str(key_date, "%m%y")
        .unwrap()
        .with_timezone(&Utc);
    let date_now = Local::now().with_timezone(&Utc);

    // check if expired or more than 2 years in the future
    if expiry_date < date_now || expiry_date > date_now + Duration::days(730) {
        Err(HttpResponse::Forbidden().into())
    }

    match String::from_utf8(bytes.to_vec()) {
        Ok(raw_board) => {
            // parse timestamp

            let fragment = Html::parse_fragment(&raw_board);
            let element: Option<Element> = fragment.select(&TIME_SELECTOR).take(1).collect();

            if let Some(time) = element {
                if let Some(attr) = time
                    .attrs()
                    .find(|(key, val)| key.to_lowercase() == "datetime")
                {
                    let datetime = chrono::DateTime::parse_from_rfc3339(attr.1)
                        .unwrap_or(chrono::DateTime::default())
                        .naive_utc();

                    let date_utc = DateTime::from_utc(datetime, Utc);

                    // if not in the future, and within 22 days
                    if date_utc < date_now
                        || date_utc.signed_duration_since(date_now) < chrono::Duration::days(22)
                    {
                    }
                }
            }
            Err(HttpResponse::BadRequest().into())
        }
        Err(_) => Err(HttpResponse::BadRequest().into()),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    log::i("starting http listener");
    HttpServer::new(|| App::new().service(index))
        .bind((CONFIG.http_listen.url.as_str(), CONFIG.http_listen.port))?
        .run()
        .await
}
