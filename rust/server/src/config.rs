// pxl-server/config.rs

use std::env::{self, VarError};

pub struct Config {
    pub host: String,
    pub port: u16,
    pub max_body_sz: usize,
    pub admin_token: String,
    pub db_url: String,
    pub db_pool_sz: usize,
}

/// A server's own variable names; distinct prefixes keep two local servers off one port.
pub struct Env<'a> {
    pub prefix: &'a str,
    pub default_port: u16,
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

fn must_int(key: &str, fallback: u64, min: u64, max: u64) -> Result<u64, String> {
    let raw = match get(key)? {
        Some(v) if !v.is_empty() => v,
        _ => return Ok(fallback),
    };
    let n: u64 = raw.parse().map_err(|_| err(key, "invalid int"))?;
    if n < min {
        return Err(err(key, "range min"));
    }
    if n > max {
        return Err(err(key, "range max"));
    }
    Ok(n)
}

impl Config {
    pub fn from_env(env: &Env) -> Result<Config, String> {
        let max_body_sz_mb = must_int("MAX_BODY_SZ_MB", 1, 1, 1024)?;

        Ok(Config {
            host: get(&format!("{}_HOST", env.prefix))?
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "0.0.0.0".to_string()),
            port: must_int(
                &format!("{}_PORT", env.prefix),
                env.default_port as u64,
                1,
                65535,
            )? as u16,
            max_body_sz: (max_body_sz_mb as usize) << 20,
            // Required even with nothing mounted under `internal/`.
            admin_token: must("ADMIN_TOKEN")?,
            db_url: must(env.db_url_key)?,
            db_pool_sz: must_int("DB_POOL_SIZE", 8, 1, 1024)? as usize,
        })
    }
}
