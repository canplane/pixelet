// pxl-server/config.rs

use std::env::{self, VarError};

pub struct Config {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) max_body_size: usize,
    pub(crate) admin_token: String,
    pub(crate) db_url: String,
    pub(crate) db_pool_size: usize,
}

/// A server's own variable names.
pub struct Env<'a> {
    pub prefix: &'a str,
    pub db_url_key: &'a str,
}

fn err(key: &str, what: &str) -> String {
    format!("env:{key}:{what}")
}

fn get(key: &str) -> Result<Option<String>, String> {
    match env::var(key) {
        Ok(v) => Ok(Some(v)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(err(key, "not unicode")),
    }
}

fn must(key: &str) -> Result<String, String> {
    match get(key)? {
        Some(v) if !v.is_empty() => Ok(v),
        Some(_) => Err(err(key, "empty")),
        None => Err(err(key, "missing")),
    }
}

fn must_int(key: &str, min: u64, max: u64) -> Result<u64, String> {
    let n: u64 = must(key)?.parse().map_err(|_| err(key, "invalid int"))?;
    if n < min {
        return Err(err(key, "range min"));
    }
    if n > max {
        return Err(err(key, "range max"));
    }
    Ok(n)
}

impl Config {
    pub(crate) fn from_env(env: &Env) -> Result<Config, String> {
        Ok(Config {
            host: must(&format!("{}_HOST", env.prefix))?,
            port: must_int(&format!("{}_PORT", env.prefix), 1, 65535)? as u16,
            max_body_size: (must_int("MAX_BODY_SIZE_MB", 1, 1024)? as usize) << 20,
            // Required even with nothing mounted under `internal/`.
            admin_token: must("ADMIN_TOKEN")?,
            db_url: must(env.db_url_key)?,
            db_pool_size: must_int("DB_POOL_SIZE", 1, 1024)? as usize,
        })
    }
}
