// pxl-geo-bake/bin/solve_decks.rs

//! Every deck's heights, solved over the whole geo DB into `overture_deck`.
//! Rerun after the transportation ingest or a solve change.

use std::time::Instant;

use pxl_geo_bake::coverage::Overture;
use pxl_geo_bake::deck::solve;
use pxl_geo_spec::table::{BASE_WATER, DECK, TRANSPORTATION};

fn run() -> Result<(), String> {
    // Named, since deck/source.rs runs its statements inline.
    let (source, mut client) = Overture::open(&[BASE_WATER, TRANSPORTATION])?;
    source.forget(&mut client, DECK)?;
    let started = Instant::now();
    let log = |line: &str| println!("[solve/deck] {line} ({:.0}s)", started.elapsed().as_secs_f64());
    solve(&mut client, source.url(), log).map_err(|e| e.to_string())?;
    source.stamp(&mut client, DECK)?;
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[solve/deck] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
