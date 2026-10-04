// pxl-geo-bake/bin/bake_geocode_tiles.rs

//! Reverse-geocode pages: a dictionary of places, then one object per group
//! of pages. The dictionary is written first from one ordered read, so a
//! resumed run numbers places the same way.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin bake_geocode_tiles -- [flags]
//!
//!   --out <dir>          output directory (default ./out/geocode/v<format>.<release>)
//!   --land-cache <path>  terrain's own land_pages.json (default ./out/terrain/land_pages.json)
//!   --concurrency <n>    parallel fetches, one connection each (default 8)
//!   --limit <n>          stop after n groups (smoke test)
//!   --force              re-fetch groups that already have an object or a marker

use std::path::PathBuf;
use std::process::ExitCode;

use postgres::Client;
use pxl_spec::bake::place_page::{encode_place_group, DICT_OBJECT, GROUP_SHAMT};
use pxl_spec::bake::published::{version_name, FORMAT};

use pxl_geo_bake::coverage::Overture;
use pxl_geo_bake::geocode::provider::overture::{self, GeocodeProvider};
use pxl_geo_bake::geocode::PlaceDict;
use pxl_geo_bake::page::bake::{self, Bake};
use pxl_geo_bake::page::walk::Page;
use pxl_geo_bake::terrain::leaf;
use pxl_geo_bake::util::{pack, write_atomic, Level};

struct Worker {
    client: Client,
    provider: GeocodeProvider,
}

fn run() -> Result<(), String> {
    let (source, mut client) = Overture::reading(&[&overture::READS, &leaf::GROUND_READS])?;

    let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
    let dict = PlaceDict::of(provider.dictionary(&mut client).map_err(|e| e.to_string())?);
    println!("[bake/geocode] {} places in the dictionary", dict.places.len());
    let packed_dict = pack(&dict.packet().map_err(|e| format!("{e:?}"))?, Level::Stored).map_err(|e| e.to_string())?;
    drop(client);

    bake::bake(
        &Bake {
            kind: "geocode",
            out: PathBuf::from(format!("./out/geocode/{}", version_name(FORMAT.geocode, &source.release))),
            group_shamt: GROUP_SHAMT,
            every: 200,
            content_type: "application/octet-stream",
            format: Some("xtp place dictionary and groups (encode_place_dict, encode_place_group)"),
            source: &source,
        },
        |out| write_atomic(&out.join(DICT_OBJECT), &packed_dict).map_err(|e| e.to_string()),
        || {
            let mut client = source.connect()?;
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
