// pxl-geo-bake/bin/bake_geocode_tiles.rs

//! Reverse-geocode tiles: a dictionary of divisions, then one object per
//! group of tiles. The dictionary is written first from one ordered read, so
//! a resumed run numbers divisions the same way.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin bake_geocode_tiles -- [flags]
//!
//!   --out <dir>          output directory (default ./out/geocode/v<format>.<UTC day>.<n>, the day's next; name the folder to resume one)
//!   --land-cache <path>  terrain's own land_tiles.json (default ./out/terrain/land_tiles.json)
//!   --concurrency <n>    parallel fetches, one connection each (default 8)
//!   --limit <n>          stop after n groups (smoke test)
//!   --force              re-fetch groups that already have an object or a marker

use std::process::ExitCode;

use postgres::Client;
use pxl_spec::bake::division_tile::{encode_division_group, DICT_OBJECT, GROUP_SHAMT};
use pxl_spec::bake::published::FORMAT;

use pxl_geo_bake::coverage::Overture;
use pxl_geo_bake::geocode::provider::overture::{self, GeocodeProvider};
use pxl_geo_bake::geocode::DivisionDict;
use pxl_geo_bake::terrain::leaf;
use pxl_geo_bake::tile::bake::{self, Bake};
use pxl_geo_bake::tile::walk::Tile;
use pxl_geo_bake::util::{next_version, pack, write_atomic, Level};

struct Worker {
    client: Client,
    provider: GeocodeProvider,
}

fn run() -> Result<(), String> {
    let reads: [&[&str]; 2] = [&overture::READS, &leaf::GROUND_READS];
    let (source, mut client) = Overture::reading(&reads, &pxl_geo_bake::coverage::Coverage::of_source()?.shards().collect())?;

    let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
    let dict = DivisionDict::of(provider.dictionary(&mut client).map_err(|e| e.to_string())?);
    println!("[bake/geocode] {} divisions in the dictionary", dict.divisions.len());
    let packed_dict = pack(&dict.packet().map_err(|e| format!("{e:?}"))?, Level::Stored).map_err(|e| e.to_string())?;
    drop(client);

    bake::bake(
        &Bake {
            kind: "geocode",
            out: next_version("./out/geocode", FORMAT.geocode),
            group_shamt: GROUP_SHAMT,
            every: 200,
            content_type: "application/octet-stream",
            format: Some("xtp division dictionary and groups (encode_division_dict, encode_division_group)"),
            source: &source,
            reads: &reads,
        },
        |out| write_atomic(&out.join(DICT_OBJECT), &packed_dict).map_err(|e| e.to_string()),
        || {
            let mut client = source.connect()?;
            let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
            Ok(Worker { client, provider })
        },
        |w, target| match w.provider.group(&mut w.client, &dict, &target.leaves)? {
            None => Ok(Tile::Empty),
            Some(group) => Ok(Tile::Object {
                items: group.tiles.iter().filter(|&&n| n != pxl_spec::bake::division_tile::NONE).count() as u64,
                raw: encode_division_group(&group).map_err(|e| format!("{e:?}"))?,
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
