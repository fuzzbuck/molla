use crate::log;
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub http_listen: HttpListenConfig,
    pub protocol: ProtocolConfig,
    pub simple_root_page: RootPageConfig,
    pub denylist: DenylistConfig,
    pub sled_dir: String,
}

#[derive(Deserialize)]
pub struct HttpListenConfig {
    pub url: String,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct ProtocolConfig {
    pub board_expire_days: i64,
}

#[derive(Deserialize, Clone)]
pub struct RootPageConfig {
    pub enabled: bool,
    pub administrators: Vec<String>,
    pub title: String,
    pub header: String,
    pub description: String,
    pub contact: String,
    pub contact_href: String,
}

#[derive(Deserialize)]
pub struct DenylistConfig {
    pub url: String,
    pub update_rate_ms: u64,
}

pub(crate) static CONFIG: Lazy<Config> = Lazy::new(|| {
    let search_paths = vec![
        "./molla.toml",
        "./config/molla.toml",
        "./configs/molla.toml",
    ];

    let mut builder: Figment = Figment::new();
    for path in search_paths.clone() {
        builder = builder.merge(Toml::file(path).nested());
    }

    let config: Config = builder
        .merge(Env::prefixed("MOLLA_"))
        .select(if cfg!(debug_assertions) {
            "debug"
        } else {
            "default"
        })
        .extract()
        .unwrap_or_else(|e| {
            log::e(
                "config.rs::CONFIG",
                &format!(
                    "configuration error. search paths: {:?}\n{:?}",
                    search_paths, e
                ),
                2,
            );
            panic!();
        });

    log::i("config loaded successfully");

    config
});
