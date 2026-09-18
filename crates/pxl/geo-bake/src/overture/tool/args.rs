// pxl-geo-bake/overture/tool/args.rs

//! The two flags every Overture ingest takes, the connection they all open,
//! and the catalog row they all write.
//!
//! A tool here is the command line and nothing else: the theme's table, its
//! columns and the mapping from a feature's properties onto them are its own,
//! and everything around those is `overture::ingest`.

use std::path::PathBuf;
use std::process::ExitCode;

use postgres::{Client, NoTls};

/// Mainland plus Jeju, the far western islands, Ulleungdo and Dokdo — the
/// same coverage infra/README.md's elevation section documents for this
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
        let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing".to_string())?;
        println!("[ingest] bbox={} out={}", self.bbox, self.out.display());
        Client::connect(&url, NoTls).map_err(|e| e.to_string())
    }

    /// The catalog row, written once the table is filled: which bbox it was
    /// pulled with and which Overture release it is from.
    pub fn record(&self, client: &mut Client, table: &str, count: i64) -> Result<(), String> {
        let cli = std::path::Path::new("./out/overture/.venv/bin/overturemaps");
        let release = super::super::ingest::release(cli)?;
        super::super::ingest::record_catalog(client, table, &self.bbox, &release, count)?;
        println!("[ingest/{table}] catalog: release {release}");
        Ok(())
    }

    pub fn fail(msg: &str) -> ExitCode {
        eprintln!("[ingest] {msg}");
        ExitCode::FAILURE
    }
}
