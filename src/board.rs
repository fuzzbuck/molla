use crate::{log, Spring83Key, CONFIG};
use ammonia::clean;
use anyhow::Error;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use pulldown_cmark::html::push_html;
use pulldown_cmark::{Options, Parser};
use serde::{Deserialize, Serialize};
use sled::{Db, IVec};

static DB: Lazy<Db> = Lazy::new(|| {
    let db = sled::open(&CONFIG.sled_dir).unwrap_or_else(|e| {
        log::e("main.rs::TREE::open", format!("{:?}", e), 3);
        panic!();
    });
    db
});

#[derive(Serialize, Deserialize)]
pub(crate) struct Board {
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) content: String,
}

pub(crate) fn sanitize_html(text: String) -> String {
    let ops = Options::all();
    let mut parser = Parser::new_ext(&text, ops);
    let mut unsafe_html = String::new();

    push_html(&mut unsafe_html, parser);
    clean(&*unsafe_html)
}

impl Board {
    /// Fetch an existing board from DB
    pub(crate) fn fetch(key_str: &str) -> Option<Self> {
        let data = DB.get(key_str).ok().unwrap_or(None);
        if let Some(data) = data {
            if let Ok(board) = serde_json::from_slice(&data.to_vec()) {
                board
            }
        }
        None
    }
    /// Put a board into DB [consumes self]
    pub(crate) fn put(self, key_str: &str) -> Result<(), Error> {
        DB.insert(key_str, serde_json::to_vec(&self)?)?;

        Ok(())
    }
}
