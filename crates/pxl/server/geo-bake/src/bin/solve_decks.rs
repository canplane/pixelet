// pxl-geo-bake/bin/solve_decks.rs

//! Every deck's heights, solved over the whole geo DB and written to
//! `overture_deck` (pxl-geo-bake deck.rs), which the terrain bake reads. It
//! runs after the transportation ingest and before a terrain bake, and again
//! whenever either the ingest or the solve changes.

use std::time::Instant;

use pxl_geo_bake::deck::solve;

fn run() -> Result<(), String> {
    let url = std::env::var("DB_GEO_URL").map_err(|_| "DB_GEO_URL required")?;
    let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let log = |line: &str| println!("[solve/deck] {line} ({:.0}s)", started.elapsed().as_secs_f64());
    let s = solve(&mut client, &url, log).map_err(|e| e.to_string())?;
    log(&format!("{} decks in {} structures, {} points: overture_deck", s.pieces, s.structures, s.points));
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
