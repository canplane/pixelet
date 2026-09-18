// pxl-geo/config.rs

use std::env::{self, VarError};

pub struct Config {
    pub host: String,
    pub port: u16,
    pub max_body_sz: usize,
    pub admin_token: String,
    pub db_geo_url: String,
    pub db_pool_size: usize,
}

/// `env:KEY:what was wrong`, so a startup failure names the variable rather
/// than only the shape of the value.
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
    /// Each server reads its own root's names — `GEO_*` here, `API_*` in
    /// pxl-api — so that running both locally cannot collide on one port by
    /// default. In a deployment each has a container to itself and both listen
    /// on 3000.
    pub fn from_env() -> Result<Config, String> {
        let max_body_sz_mb = must_int("MAX_BODY_SZ_MB", 1, 1, 1024)?;

        Ok(Config {
            host: get("GEO_HOST")?.filter(|v| !v.is_empty()).unwrap_or_else(|| "0.0.0.0".to_string()),
            port: must_int("GEO_PORT", 3000, 1, 65535)? as u16,
            max_body_sz: (max_body_sz_mb as usize) << 20,
            // Gates /geo/internal/*, and is required even while nothing is
            // mounted there — see main.rs's own note on why the gate covers
            // the prefix rather than a list of routes.
            admin_token: must("ADMIN_TOKEN")?,
            db_geo_url: must("DB_GEO_URL")?,
            db_pool_size: must_int("DB_POOL_SIZE", 8, 1, 1024)? as usize,
        })
    }
}
