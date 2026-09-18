// pxl-asset-bake/tool/download_texture_pack.rs

//! The texture pack, normalized.
//!
//! For every entry in `asset/cube/manifest.json`: fetch the URL only if
//! `asset/cube/original/` does not already hold a valid copy from that same
//! URL, then always re-normalize to a `TILE_SIZE` square opaque RGBA PNG under
//! `asset/cube/{terrain,block}/`. The client fetches those by content hash from
//! the bucket, so a changed image is followed by `ingest:asset`.
//!
//! Re-runnable and idempotent for an unchanged manifest: the cached original
//! is the resize source, so a re-run never reaches the network merely to
//! re-normalize.
//!
//!   deno task download:pack

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::{ImageFormat, RgbaImage};

use pxl_asset_bake::pack::{self, Item, Style, TILE_SIZE};

const RETRY_ATTEMPTS: u32 = 3;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
}

/// A few attempts with a growing pause, because the sources are third-party
/// URLs and one refused connection is not a reason to fail a whole pack.
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

/// The cached original, if the manifest's URL for this item still matches what
/// produced it. `None` — never fetched, or the manifest moved on — means the
/// caller has to fetch.
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
            // Lanczos3 band-limits before downsampling, where nearest would
            // just drop samples and alias the fine painted detail these
            // sources were chosen for into pixelation.
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

    // What landed on disk, not the buffer that was meant to: an encoder that
    // wrote a different size or left an alpha behind would otherwise reach the
    // bucket as a layer the array texture cannot take.
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
    let cube = root().join("asset/cube");
    let items = pack::load(&cube)?;
    let mut state = pack::read_state(&cube);
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
    pack::write_state(&cube, &state)?;
    println!("[pack] {cached} from cached originals, {fetched} fetched, {failed} failed");
    Ok(failed)
}
