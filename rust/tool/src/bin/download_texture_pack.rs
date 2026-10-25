// pxl-tool/bin/download_texture_pack.rs

//! Fetches each `asset/raw/cube/manifest.json` entry not already cached there
//! and normalizes it to a `TILE_SIZE` opaque PNG in `asset/content/cube/`.
//! Follow with `ingest:asset`.
//!
//!   make download-pack

use std::process::ExitCode;

use image::{ImageFormat, RgbaImage};

use pxl_tool::asset::pack::{self, Item, Style, TILE_SIZE};
use pxl_tool::root;

const RETRY_ATTEMPTS: u32 = 3;

fn fetch_with_retry(url: &str) -> Result<Vec<u8>, String> {
    let mut last = String::new();
    for attempt in 1..=RETRY_ATTEMPTS {
        match ureq::get(url).call() {
            Ok(res) => {
                let mut body = Vec::new();
                res.into_reader().read_to_end(&mut body).map_err(|e| e.to_string())?;
                if body.is_empty() {
                    last = "empty response body".to_string();
                } else {
                    return Ok(body);
                }
            }
            Err(e) => last = e.to_string(),
        }
        if attempt < RETRY_ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_millis(400 * attempt as u64));
        }
    }
    Err(last)
}

fn cached_original(item: &Item, state: &pack::State) -> Option<RgbaImage> {
    if state.get(&item.file).map(String::as_str) != Some(item.url.as_str()) {
        return None;
    }
    let bytes = std::fs::read(&item.original).ok()?;
    image::load_from_memory_with_format(&bytes, ImageFormat::Png).ok().map(|i| i.to_rgba8())
}

enum Outcome {
    Cached,
    Fetched,
}

fn process(item: &Item, state: &mut pack::State) -> Result<Outcome, String> {
    let (mut img, outcome) = match cached_original(item, state) {
        Some(img) => (img, Outcome::Cached),
        None => {
            let raw = fetch_with_retry(&item.url).map_err(|e| format!("download failed: {e}"))?;
            std::fs::create_dir_all(item.original.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
            std::fs::write(&item.original, &raw).map_err(|e| e.to_string())?;
            state.insert(item.file.clone(), item.url.clone());
            let img = image::load_from_memory_with_format(&raw, ImageFormat::Png).map_err(|e| format!("not a decodable PNG: {e}"))?.to_rgba8();
            (img, Outcome::Fetched)
        }
    };

    if img.width() != TILE_SIZE || img.height() != TILE_SIZE {
        if pack::looks_like_sprite_sheet(img.width(), img.height()) {
            return Err(format!(
                "suspected sprite sheet / non-square source ({}x{}) — needs manual review, not auto-resized",
                img.width(),
                img.height()
            ));
        }
        img = match item.style {
            Style::Painted => image::imageops::resize(&img, TILE_SIZE, TILE_SIZE, image::imageops::FilterType::Lanczos3),
            Style::Legacy => {
                let (w, h) = (img.width(), img.height());
                let raw = pack::nearest_resize(img.as_raw(), w, h, TILE_SIZE, TILE_SIZE);
                RgbaImage::from_raw(TILE_SIZE, TILE_SIZE, raw).ok_or("nearest resize produced the wrong length")?
            }
        };
    }

    pack::flatten_alpha(img.as_mut(), item.palette_hex.as_deref());

    std::fs::create_dir_all(item.dest.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
    img.save_with_format(&item.dest, ImageFormat::Png).map_err(|e| format!("{}: {e}", item.dest.display()))?;

    // Check what landed on disk, not the buffer: a wrong size or a leftover alpha
    // would reach the bucket as a layer the array texture cannot take.
    let written = image::open(&item.dest).map_err(|e| format!("post-write: {e}"))?.to_rgba8();
    if written.width() != TILE_SIZE || written.height() != TILE_SIZE {
        return Err(format!("post-write validation failed: got {}x{}", written.width(), written.height()));
    }
    if written.pixels().any(|p| p.0[3] != 255) {
        return Err("post-write validation failed: unflattened alpha remains".to_string());
    }
    Ok(outcome)
}

fn main() -> ExitCode {
    match run() {
        Ok(0) => ExitCode::SUCCESS,
        Ok(failed) => {
            eprintln!("[pack] {failed} entries failed");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("[pack] {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<usize, String> {
    let (raw, content) = (root().join("asset/raw/cube"), root().join("asset/content/cube"));
    let items = pack::load(&raw, &content)?;
    let mut state = pack::read_state(&raw);
    println!("[pack] {} entries", items.len());

    let (mut cached, mut fetched, mut failed) = (0, 0, 0);
    for item in &items {
        match process(item, &mut state) {
            Ok(Outcome::Cached) => cached += 1,
            Ok(Outcome::Fetched) => fetched += 1,
            Err(e) => {
                failed += 1;
                eprintln!("[pack/{}] {e}", item.label);
            }
        }
    }
    pack::write_state(&raw, &state)?;
    println!("[pack] {cached} from cached originals, {fetched} fetched, {failed} failed");
    Ok(failed)
}
