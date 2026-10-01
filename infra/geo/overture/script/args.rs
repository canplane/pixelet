//! The two flags every Overture ingest takes, the connection they all open,
//! and the catalog row they all write.
//!
//! A tool here is the command line and nothing else: the theme's table, its
//! columns and the mapping from a feature's properties onto them are its own,
//! and everything around those is `overture`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use postgres::{Client, NoTls};

/// The repository's own `.env`, as `setup-elevation.sh` reads it beside these:
/// running a script is the whole interface, so it cannot need a wrapper to put
/// one variable in front of it. The environment wins where it already has the
/// name, which is what lets CI set it without a file.
///
/// The path is taken from this file rather than the working directory, so the
/// script runs from anywhere.
fn db_geo_url() -> Result<String, String> {
    if let Ok(url) = std::env::var("DB_GEO_URL") {
        return Ok(url);
    }
    let dotenv = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../.env");
    let text = std::fs::read_to_string(&dotenv).map_err(|_| format!("env:DB_GEO_URL:missing, and no {}", dotenv.display()))?;

    let mut vars: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        let value = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value);
        vars.insert(key.trim(), value);
    }
    let raw = vars.get("DB_GEO_URL").ok_or_else(|| format!("env:DB_GEO_URL:missing, and {} does not set it", dotenv.display()))?;

    // `DB_GEO_URL` is written out of the other keys — `${DB_GEO_USER}` and the
    // rest — because a compose file reads the same file and wants the parts.
    // A shell would expand them on the way in; nothing does that here.
    let mut out = String::with_capacity(raw.len());
    let mut rest = *raw;
    while let Some(at) = rest.find("${") {
        out.push_str(&rest[..at]);
        let Some(end) = rest[at..].find('}') else { break };
        let name = &rest[at + 2..at + end];
        out.push_str(vars.get(name).copied().unwrap_or(""));
        rest = &rest[at + end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Mainland plus Jeju, the far western islands, Ulleungdo and Dokdo — the
/// same coverage infra/geo/README.md's GEDTM30 section documents for this
/// project's own Korea box.
const KR_BBOX: &str = "124,33,132,39";

pub struct Args {
    pub bbox: String,
    pub out: PathBuf,
}

impl Args {
    pub fn parse() -> Result<Args, String> {
        let mut args = Args {
            bbox: KR_BBOX.to_string(),
            out: PathBuf::from("./out/overture"),
        };
        let argv: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < argv.len() {
            let next = |i: &mut usize| -> Result<String, String> {
                *i += 1;
                argv.get(*i).cloned().ok_or_else(|| format!("{} wants a value", argv[*i - 1]))
            };
            match argv[i].as_str() {
                "--bbox" => args.bbox = next(&mut i)?,
                "--out" => args.out = PathBuf::from(next(&mut i)?),
                other => return Err(format!("unknown flag: {other}")),
            }
            i += 1;
        }
        Ok(args)
    }

    pub fn connect(&self) -> Result<Client, String> {
        let url = db_geo_url()?;
        println!("[ingest] bbox={} out={}", self.bbox, self.out.display());
        Client::connect(&url, NoTls).map_err(|e| e.to_string())
    }

    /// The catalog row, written once the table is filled: which bbox it was
    /// pulled with and which Overture release it is from.
    pub fn record(&self, client: &mut Client, table: &str, ingested: &crate::overture::Ingested) -> Result<(), String> {
        crate::overture::record_catalog(client, table, &self.bbox, &ingested.release, ingested.count)?;
        println!("[ingest/{table}] catalog: release {}", ingested.release);
        Ok(())
    }

    pub fn fail(msg: &str) -> ExitCode {
        eprintln!("[ingest] {msg}");
        ExitCode::FAILURE
    }
}
