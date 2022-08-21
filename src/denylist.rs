use crate::{log, Spring83Key, CONFIG};
use dashmap::DashSet;
use iter_tools::Itertools;
use once_cell::sync::Lazy;

pub(crate) struct Denylist;

static DENYLIST: Lazy<DashSet<String>> = Lazy::new(|| DashSet::new());

impl Denylist {
    pub(crate) fn check_key(key: &str) -> bool {
        DENYLIST.contains(key)
    }
    fn update(denylist: String) {
        denylist.lines().for_each(|line| {
            let line = line.trim();
            let parsed_line = line.split_once('#').unwrap_or((line, "")).0;

            // if valid line (strip comments) && not already in denylist, add it
            if Spring83Key::validate(parsed_line) && !Self::check_key(parsed_line) {
                DENYLIST.insert(parsed_line.to_string());
            }
        })
    }
    pub(crate) async fn heartbeat() {
        match reqwest::get(&CONFIG.denylist.url).await {
            Ok(resp) => match resp.text().await {
                Ok(body_str) => {
                    Self::update(body_str);
                    log::i(format!("updated denylist: {} entries", DENYLIST.len()));
                }
                Err(e) => log::w(format!(
                    "[WILL RETRY] denylist body response is bad\n{:?}",
                    e
                )),
            },
            Err(e) => log::w(format!(
                "[WILL RETRY] failed to fetch denylist from url {}\n{:?}",
                &CONFIG.denylist.url, e
            )),
        }
    }
}
