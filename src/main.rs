extern crate core;

mod board;
mod config;
mod denylist;
mod log;
mod spring83key;

use actix_cors::Cors;

use crate::board::Board;
use crate::config::CONFIG;
use crate::denylist::Denylist;

use actix_web::http::header::HeaderValue;
use actix_web::http::{Method, StatusCode};

use actix_web::web::Bytes;
use actix_web::{
    get, http, options, put, web, App, HttpRequest, HttpResponse, HttpResponseBuilder, HttpServer,
    Responder,
};

use chrono::{DateTime, Duration, FixedOffset, Local, NaiveDate, Utc};

use handlebars::Handlebars;
use http::header;
use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Selector};
use serde_json::json;
use spring83key::Spring83Key;
use tokio::time::sleep;

static TIME_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("time").unwrap());

static SIMPLE_ROOT_PAGE: Lazy<String> = Lazy::new(|| {
    let p = CONFIG.simple_root_page.clone();
    let (title, header, desc, people, contact, href) = (
        p.title,
        p.header,
        p.description,
        p.administrators,
        p.contact,
        p.contact_href,
    );

    let mut reg = Handlebars::new();
    reg.register_template_file("template", "./root_page.html")
        .expect("handlebars templating fail");

    let data = json!({
        "title": title,
        "header": header,
        "description": desc,
        "people": people,
        "contact": contact,
        "contact_href": href
    });

    reg.render("template", &data)
        .expect("handlebars rendering fail")
});

#[get("/")]
async fn index() -> impl Responder {
    HttpResponseBuilder::new(StatusCode::from_u16(200).unwrap())
        .content_type("text/html; charset=UTF-8")
        .body(SIMPLE_ROOT_PAGE.clone())
}

#[get("/{key}")]
async fn get(key: web::Path<String>, _req: HttpRequest) -> impl Responder {
    if let Some(board) = Board::fetch(&key.into_inner()) {
        if let Some(if_modified_since) = _req.headers().get("If-Modified-Since") {
            let date_str = if_modified_since.to_str().unwrap_or("");
            let modified_date = DateTime::parse_from_rfc2822(date_str)
                .unwrap_or(DateTime::default())
                .with_timezone(&Utc);

            if board.timestamp < modified_date {
                return HttpResponseBuilder::new(StatusCode::from_u16(304).unwrap());
            }
        }
        if Local::now()
            .with_timezone(&Utc)
            .signed_duration_since(board.timestamp)
            < Duration::days(CONFIG.protocol.board_expire_days)
        {
            HttpResponseBuilder::new(StatusCode::from_u16(200).unwrap())
                .content_type("text/html; charset=UTF-8")
                .body(board.content);
        }
    }
    // expired or non existent
    HttpResponseBuilder::new(StatusCode::from_u16(404).unwrap())
}

#[put("/{key}")]
async fn put(key: web::Path<String>, bytes: Bytes, req: HttpRequest) -> impl Responder {
    // check board size first
    if bytes.len() > 2217 {
        return HttpResponse::PayloadTooLarge();
    }

    let key_str = key.into_inner();

    // run through denylist
    if Denylist::check_key(&key_str) {
        return HttpResponse::Forbidden();
    }

    // validate regex
    if Spring83Key::validate(&key_str) {
        // get signature header
        let signature_header = req.headers().get("Spring-Signature");
        if signature_header.is_none() {
            return HttpResponse::BadRequest();
        }
        let signature_str = signature_header.unwrap().to_str().unwrap_or("");

        let key = Spring83Key::from_hex(&key_str, signature_str).ok();
        return if let Some(key) = key {
            if key.expired_or_too_far_in_future() || !key.verify() {
                return HttpResponse::Unauthorized();
            }

            match String::from_utf8(bytes.to_vec()) {
                Ok(raw_board) => {
                    // parse timestamp
                    let fragment = Html::parse_fragment(&raw_board);
                    let element: Option<ElementRef> = fragment
                        .select(&TIME_SELECTOR)
                        .collect::<Vec<ElementRef>>()
                        .first()
                        .cloned();

                    if let Some(time) = element {
                        if let Some(attr) = time.value().clone().attr("datetime") {
                            let date_then = chrono::DateTime::parse_from_rfc3339(attr)
                                .unwrap_or_default()
                                .with_timezone(&Utc);

                            // if not in the future, and within 22 days
                            let date_now = Local::now().with_timezone(&Utc);
                            if date_then < date_now
                                || date_then.signed_duration_since(date_now)
                                    < chrono::Duration::days(22)
                            {
                                let board = Board {
                                    timestamp: date_then,
                                    content: board::sanitize_html(raw_board),
                                };
                                return match board.put(&key_str) {
                                    Ok(_) => HttpResponse::Ok(),
                                    Err(_) => HttpResponse::InternalServerError(),
                                };
                            }
                        }
                    }
                    HttpResponse::BadRequest()
                }
                Err(_) => HttpResponse::BadRequest(),
            }
        } else {
            HttpResponse::BadRequest()
        };
    };

    HttpResponse::Ok()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    log::i("starting denylist heartbeat");
    let _ = tokio::spawn(async move {
        loop {
            Denylist::heartbeat().await;
            sleep(std::time::Duration::from_millis(
                CONFIG.denylist.update_rate_ms,
            ))
        }
    });

    log::i("starting http listener");
    HttpServer::new(|| {
        let cors = Cors::default()
            .allowed_headers(vec![
                header::CONTENT_TYPE,
                header::IF_MODIFIED_SINCE,
                header::from_one_raw_str(Some(&HeaderValue::from_str("Spring-Signature").unwrap()))
                    .unwrap(),
                header::from_one_raw_str(Some(&HeaderValue::from_str("Spring-Version").unwrap()))
                    .unwrap(),
            ])
            .expose_headers([
                header::CONTENT_TYPE,
                header::LAST_MODIFIED,
                header::from_one_raw_str(Some(&HeaderValue::from_str("Spring-Signature").unwrap()))
                    .unwrap(),
                header::from_one_raw_str(Some(&HeaderValue::from_str("Spring-Version").unwrap()))
                    .unwrap(),
            ])
            .allowed_methods(vec![Method::GET, Method::OPTIONS, Method::PUT])
            .allow_any_origin();

        App::new()
            .wrap(cors)
            .service(get)
            .service(put)
            .service(index)

        /*
        if CONFIG.simple_root_page.enabled {
            app = app.service(index)
        }

             */
    })
    .bind((CONFIG.http_listen.url.as_str(), CONFIG.http_listen.port))?
    .run()
    .await
}
