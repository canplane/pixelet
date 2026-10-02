// pxl-geo-bake/bin/bake_geocode_tiles.rs

//! Reverse-geocode pages: a dictionary of places, then one object per group
//! of pages. The dictionary is written first from one ordered read, so a
//! resumed run numbers places the same way.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin bake_geocode_tiles -- [flags]
//!
//!   --out <dir>          output directory (default ./out/geocode/<published version>)
//!   --land-cache <path>  terrain's own land_pages.json (default ./out/terrain/land_pages.json)
//!   --concurrency <n>    parallel fetches, one connection each (default 8)
//!   --limit <n>          stop after n groups (smoke test)
//!   --force              re-fetch groups that already have an object or a marker

use std::path::PathBuf;
use std::process::ExitCode;

use postgres::{Client, NoTls};
use pxl_spec::bake::place_page::{encode_place_group, DICT_OBJECT, GROUP_SHAMT};
use pxl_spec::bake::published::PUBLISHED_VERSION;

use pxl_geo_bake::geocode::provider::overture::GeocodeProvider;
use pxl_geo_bake::geocode::PlaceDict;
use pxl_geo_bake::page::bake::{self, Bake};
use pxl_geo_bake::page::walk::Page;
use pxl_geo_bake::util::{pack, write_atomic, Level};
use pxl_geo_spec::table;

struct Worker {
    client: Client,
    provider: GeocodeProvider,
}

fn run() -> Result<(), String> {
    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;

    let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;
    let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
    let dict = PlaceDict::of(provider.dictionary(&mut client).map_err(|e| e.to_string())?);
    println!("[bake/geocode] {} places in the dictionary", dict.places.len());
    let packed_dict = pack(&dict.packet().map_err(|e| format!("{e:?}"))?, Level::Stored).map_err(|e| e.to_string())?;
    drop(client);

    bake::bake(
        &Bake {
            kind: "geocode",
            out: PathBuf::from(format!("./out/geocode/{}", PUBLISHED_VERSION.geocode)),
            group_shamt: GROUP_SHAMT,
            every: 200,
            content_type: "application/octet-stream",
            format: Some("xtp place dictionary and groups (encode_place_dict, encode_place_group)"),
            sources: &[table::DIVISIONS, table::DIVISION_POINTS],
        },
        |out| write_atomic(&out.join(DICT_OBJECT), &packed_dict).map_err(|e| e.to_string()),
        || {
            let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;
            let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
            Ok(Worker { client, provider })
        },
        |w, target| match w.provider.group(&mut w.client, &dict, &target.leaves)? {
            None => Ok(Page::Empty),
            Some(group) => Ok(Page::Object {
                items: group.labels.len() as u64,
                raw: encode_place_group(&group).map_err(|e| format!("{e:?}"))?,
            }),
        },
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[bake/geocode] {err}");
            ExitCode::FAILURE
        }
    }
}
