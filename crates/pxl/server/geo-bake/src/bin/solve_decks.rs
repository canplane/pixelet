// pxl-geo-bake/bin/solve_decks.rs

//! Every deck's heights into `overture_deck`; `--bbox w,s,e,n` re-solves only
//! what is there, in place, to look at one place after a solve change.

use std::time::Instant;

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::coverage::Overture;
use pxl_geo_bake::deck::solve;
use pxl_geo_spec::table::{BASE_WATER, DECK, TRANSPORTATION};

fn run() -> Result<(), String> {
    let mut within = None;
    let mut flags = Flags::from_env();
    while let Some(arg) = flags.arg() {
        match arg.as_str() {
            "--bbox" => {
                let v: Vec<f64> = flags.value()?.split(',').map(|x| x.parse().map_err(|_| "bad --bbox")).collect::<Result<_, _>>()?;
                within = Some(<[f64; 4]>::try_from(v).map_err(|_| "--bbox wants w,s,e,n")?);
            }
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    // Named, since deck/source.rs runs its statements inline.
    let (source, mut client) = Overture::open(&[BASE_WATER, TRANSPORTATION])?;
    let started = Instant::now();
    let log = |line: &str| println!("[solve/deck] {line} ({:.0}s)", started.elapsed().as_secs_f64());
    // a place's re-solve leaves the whole table's stamp
    if within.is_none() {
        source.forget(&mut client, DECK)?;
    }
    solve(&mut client, source.url(), within, log).map_err(|e| e.to_string())?;
    if within.is_none() {
        source.stamp(&mut client, DECK)?;
    }
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
