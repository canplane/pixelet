// pxl-geo-bake/bin/decode_tile.rs

//! A baked terrain page, read back the way the kernel reads it.
//!
//!   decode_tile 12720250262323200                # one leaf: summary + PNGs
//!   decode_tile 3179787687624704 --level 2       # a page of a level above
//!   decode_tile 12720250262323200 --out /tmp/page
//!   decode_tile out/terrain/v7/_ocean            # a path works too
//!   decode_tile --all                            # every page of every level, checked
//!   decode_tile --all --version v7
//!
//! (`cargo run --manifest-path crates/Cargo.toml --profile server --bin
//! decode_tile -- …`, from the repository root, where `out/` is.)
//!
//! A page is read by the model's own `decode_terrain`, so a page this cannot
//! read is one the client cannot either. What it checks past that is the
//! bake's contract: the elevation is the padded page's, the ground and the
//! index lane the page's own size, nothing follows the tree, every packed
//! cell names a type its channel gives out, and the index lane holds at least
//! the page's own surface — what it says of the pages under it is the fold's
//! (pxl-spec bake/fold.rs `fold_index`), whose laws are tested there. A cell's fields are indices into the pack's
//! rows, and reading one past its row clamps to the row's last entry
//! (pxl-spec bake/pack.rs), so a writer that disagrees with the reader draws
//! the wrong ground rather than failing — which is why it is checked here.
//! A bake that drifts is caught before it is published, which is what
//! `--all` after `deno task bake:tiles` is for.
//!
//! For one page it also writes `heights.png` (the raster stretched to its own
//! range, so a flat page is flat gray), one PNG a channel in the palette's
//! colors, with `void` left black and an index the pack has no type for in
//! magenta, and `decks.png`, the decks from above with their piers white.
//! The picture is for the eye; the summary is the check.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use flate2::write::ZlibEncoder;
use flate2::{Compression, Crc};
use pxl_geo_bake::cli::Flags;
use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::decode_terrain;
use pxl_spec::bake::format::{GroundChannelKey, Occupancy};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::bake::surface::extent_of;
use pxl_spec::cell::cube::ground_pack;
use pxl_spec::cell::{color, cube_types};
use pxl_spec::space::pyramid::PAGE_CELLS;
use xpute_core::wire::xtp::{HDR_SZ, WORD_SZ};

const MAGENTA: u32 = 0xff00ff;

/// A stored page's bytes as the bake left them: zstd now, gzip in the tile
/// sets baked before it, and identity where a page was fetched from a
/// deployment, which decodes it on the way. Told apart by their first bytes.
fn read_page(path: &Path) -> Result<Vec<u8>, String> {
    let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if raw.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return zstd::decode_all(raw.as_slice()).map_err(|e| format!("zstd: {e}"));
    }
    if raw.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(raw.as_slice()).read_to_end(&mut out).map_err(|e| format!("gzip: {e}"))?;
        return Ok(out);
    }
    Ok(raw)
}

struct Page {
    ele: Vec<i16>,
    cells: Vec<GroundCell>,
    extent: Vec<[i16; 2]>,
    occupancy: Occupancy,
    decks: Decks,
}

/// The page as the kernel reads it, or what is wrong with its shape.
fn decode(pkt: &[u8]) -> Result<Page, String> {
    // A packet whose lanes are not a page's is refused by the decoder.
    let t = decode_terrain(pkt).map_err(|e| format!("not a page: {e:?}"))?;
    let (ele, cells, extent, occupancy, decks) = (t.ele.into_owned(), t.ground.cells.into_owned(), t.index.extent().to_vec(), t.index.occupancy, t.decks);
    // Nothing may follow: a page is exactly its tree, and bytes past it are a
    // writer that disagrees with the reader, which only checks that the
    // payload the header names is there.
    let at = WORD_SZ as usize;
    let payload = u32::from_le_bytes(pkt[at..at + 4].try_into().expect("a header word")) as usize;
    if payload + HDR_SZ as usize != pkt.len() {
        return Err(format!("packet is {} bytes, header says {}", pkt.len(), payload + HDR_SZ as usize));
    }
    Ok(Page { ele, cells, extent, occupancy, decks })
}

/// A packed cell's field `f` as the dense index the word holds, which may lie
/// past its row: the fault `channel_of` would hide.
fn index_of(cell: GroundCell, f: usize) -> usize {
    ((cell as u32 >> ground_pack::SHIFT[f]) & ((1 << ground_pack::BITS[f]) - 1)) as usize
}

/// A type's name in a channel, `void` and the other sentinels included.
fn name_of(channel: &str, number: u8) -> &'static str {
    cube_types::CHANNELS
        .iter()
        .filter(|(c, _)| *c == channel || *c == "common")
        .flat_map(|(_, entries)| entries.iter())
        .find(|(_, n)| *n == number)
        .map_or("?", |(name, _)| name)
}

struct Summary {
    min: i16,
    max: i16,
    mean: f64,
    /// The index lane's lowest and highest over the whole page.
    extent: [i16; 2],
    occupancy: Occupancy,
    /// Per channel in the pack's order: cells by type name.
    counts: Vec<BTreeMap<&'static str, u32>>,
    faults: Vec<String>,
}

fn summarize(page: &Page) -> Summary {
    let (min, max) = page.ele.iter().fold((i16::MAX, i16::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    let mean = page.ele.iter().map(|&h| h as f64).sum::<f64>() / page.ele.len() as f64;
    let mut counts = vec![BTreeMap::new(); ground_pack::CHANNELS.len()];
    let mut past = vec![BTreeMap::<usize, u32>::new(); ground_pack::CHANNELS.len()];
    let mut high = 0u32;
    for &cell in &page.cells {
        if cell as u32 >> ground_pack::CELL_BITS != 0 {
            high += 1;
        }
        for (f, channel) in ground_pack::CHANNELS.iter().enumerate() {
            let i = index_of(cell, f);
            match ground_pack::NUMBER[f].get(i) {
                Some(&n) => *counts[f].entry(name_of(channel, n)).or_insert(0) += 1,
                None => *past[f].entry(i).or_insert(0) += 1,
            }
        }
    }
    let mut faults = Vec::new();
    for (f, channel) in ground_pack::CHANNELS.iter().enumerate() {
        if !past[f].is_empty() {
            let list: Vec<String> = past[f].iter().map(|(i, n)| format!("{i}×{n}")).collect();
            faults.push(format!("{channel}: index(es) past the pack's row — {}", list.join(", ")));
        }
    }
    if high > 0 {
        faults.push(format!("{high} cell(s) with bits set past the pack's {} bits", ground_pack::CELL_BITS));
    }
    let short = extent_of(&page.ele, &page.cells, &page.decks).iter().zip(&page.extent).filter(|([lo, hi], [ilo, ihi])| ilo > lo || ihi < hi).count();
    if short > 0 {
        faults.push(format!("{short} index cell(s) narrower than the page's own surface"));
    }
    let extent = page.extent.iter().fold([i16::MAX, i16::MIN], |[lo, hi], &[l, h]| [lo.min(l), hi.max(h)]);
    Summary {
        min,
        max,
        mean,
        extent,
        occupancy: page.occupancy,
        counts,
        faults,
    }
}

fn print_summary(name: &str, s: &Summary) {
    println!("page {name}");
    println!("  ele  min {} max {} mean {:.2}{}", s.min, s.max, s.mean, if s.min == s.max { " (flat)" } else { "" });
    println!("  index  lowest {} highest {} occupancy {:#07b}", s.extent[0], s.extent[1], s.occupancy.bits());
    for (f, channel) in ground_pack::CHANNELS.iter().enumerate() {
        let list: Vec<String> = s.counts[f].iter().map(|(name, n)| format!("{name} {n}")).collect();
        println!("  {channel:<9} {}", list.join(", "));
    }
}

fn print_decks(decks: &Decks) {
    let tops = decks.tops.iter().fold([i16::MAX, i16::MIN], |[lo, hi], &t| [lo.min(t), hi.max(t)]);
    let piers = decks.piers.iter().filter(|&&p| p != 0).count();
    match decks.len() {
        0 => println!("  decks     none"),
        n => println!("  decks     {n}, {} points, {piers} piers, tops {}..{}", decks.points.len(), tops[0], tops[1]),
    }
}

// ---- PNG ----

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut crc = Crc::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc.sum().to_be_bytes());
}

/// A PNG of `width`×`height` pixels, `channels` bytes each (1 gray, 3 RGB).
fn png(width: u32, height: u32, channels: u8, pixels: &[u8]) -> Vec<u8> {
    let stride = (width * channels as u32) as usize;
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    for row in pixels.chunks(stride) {
        z.write_all(&[0]).and_then(|_| z.write_all(row)).expect("a write into memory");
    }
    let data = z.finish().expect("a write into memory");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, if channels == 1 { 0 } else { 2 }, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &data);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn heights_png(page: &Page, s: &Summary) -> Vec<u8> {
    let span = (s.max as f64 - s.min as f64).max(0.0);
    let gray: Vec<u8> = page
        .ele
        .iter()
        .map(|&h| if span == 0.0 { 128 } else { ((h as f64 - s.min as f64) / span * 255.0).round() as u8 })
        .collect();
    png(ELE_PADDED_SPAN, ELE_PADDED_SPAN, 1, &gray)
}

/// Channel `f` in the palette's colors.
fn channel_png(page: &Page, f: usize) -> Vec<u8> {
    let channel = GroundChannelKey::from_id(f as u32).expect("a packed field is a ground channel");
    let rgb_of = |cell: GroundCell| match ground_pack::NUMBER[f].get(index_of(cell, f)) {
        None => MAGENTA,
        Some(&n) if n == cube_types::common::void => 0,
        Some(&n) => color::ground(channel, n).map_or(MAGENTA, |s| s.rgb),
    };
    let px: Vec<u8> = page
        .cells
        .iter()
        .flat_map(|&c| {
            let v = rgb_of(c);
            [(v >> 16) as u8, (v >> 8) as u8, v as u8]
        })
        .collect();
    png(PAGE_CELLS, PAGE_CELLS, 3, &px)
}

/// The decks drawn from above: each covered cell its surface's color, the
/// higher deck where two cross, and a pier white.
fn decks_png(page: &Page) -> Vec<u8> {
    let n = PAGE_CELLS as i64;
    let mut top = vec![f64::NEG_INFINITY; (n * n) as usize];
    let mut px = vec![0u8; (n * n * 3) as usize];
    page.decks.over_grid([0.0, 0.0], 1.0, [n, n], |x, y, _, over| {
        let i = (y * n + x) as usize;
        if over.top < top[i] {
            return;
        }
        top[i] = over.top;
        let v = if over.pier { 0xffffff } else { color::ground(GroundChannelKey::TRANSPORT, over.kind).map_or(MAGENTA, |s| s.rgb) };
        px[i * 3..i * 3 + 3].copy_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, v as u8]);
    });
    png(PAGE_CELLS, PAGE_CELLS, 3, &px)
}

// ---- driver ----

/// Every stored page under a tile set: the ocean page, and each level's.
fn stored_pages(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            stored_pages(&path, out)?;
        } else if !matches!(path.extension().and_then(|e| e.to_str()), Some("part" | "empty")) {
            out.push(path);
        }
    }
    Ok(())
}

fn check_all(pages: &Path) -> ExitCode {
    let mut paths = Vec::new();
    if let Err(e) = stored_pages(pages, &mut paths) {
        eprintln!("[decode_tile] {}: {e} — bake first, or name the version", pages.display());
        return ExitCode::FAILURE;
    }
    if paths.is_empty() {
        eprintln!("[decode_tile] no pages under {} — bake first, or name the version", pages.display());
        return ExitCode::FAILURE;
    }
    let mut bad = Vec::new();
    for (k, path) in paths.iter().enumerate() {
        let name = path.strip_prefix(pages).unwrap_or(path).display().to_string();
        match read_page(path).and_then(|pkt| decode(&pkt)) {
            Ok(page) => {
                let s = summarize(&page);
                if !s.faults.is_empty() {
                    bad.push(format!("{name}: {}", s.faults.join("; ")));
                }
            }
            Err(e) => bad.push(format!("{name}: {e}")),
        }
        if (k + 1) % 1000 == 0 {
            eprintln!("[decode_tile] {} pages read, {} at fault", k + 1, bad.len());
        }
    }
    for line in &bad {
        eprintln!("  {line}");
    }
    if !bad.is_empty() {
        eprintln!("[decode_tile] {} of {} page(s) under {} at fault", bad.len(), paths.len(), pages.display());
        return ExitCode::FAILURE;
    }
    println!("[decode_tile] ok — {} page(s) under {} read as the kernel reads them", paths.len(), pages.display());
    ExitCode::SUCCESS
}

fn one(path: &Path, out: Option<PathBuf>) -> Result<bool, String> {
    let page = decode(&read_page(path)?)?;
    let s = summarize(&page);
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    print_summary(&name, &s);
    print_decks(&page.decks);
    for line in &s.faults {
        eprintln!("[decode_tile] {line}");
    }
    let dir = out.unwrap_or_else(|| Path::new("out/decoded").join(&name));
    let write = |file: String, bytes: Vec<u8>| std::fs::write(dir.join(&file), bytes).map_err(|e| format!("{file}: {e}"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    write("heights.png".into(), heights_png(&page, &s))?;
    write("decks.png".into(), decks_png(&page))?;
    for (f, channel) in ground_pack::CHANNELS.iter().enumerate() {
        write(format!("{channel}.png"), channel_png(&page, f))?;
    }
    println!("[decode_tile] wrote heights.png and a PNG a channel under {}", dir.display());
    Ok(s.faults.is_empty())
}

fn main() -> ExitCode {
    const USAGE: &str = "usage: decode_tile <key | path> [--level n] [--version v6] [--out dir] | --all [--version v6]";
    let (mut version, mut level, mut out, mut all, mut target) = (PUBLISHED_VERSION.terrain.to_string(), "0".to_string(), None, false, None);
    let mut flags = Flags::from_env();
    let read = (|| {
        while let Some(arg) = flags.arg() {
            match arg.as_str() {
                "--version" => version = flags.value()?,
                "--level" => level = flags.value()?,
                "--out" => out = Some(PathBuf::from(flags.value()?)),
                "--all" => all = true,
                _ if target.is_none() && !arg.starts_with("--") => target = Some(arg),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok::<(), String>(())
    })();
    if let Err(e) = read {
        eprintln!("{e}\n{USAGE}");
        return ExitCode::from(2);
    }
    let pages = Path::new("out/terrain").join(&version);
    if all {
        return check_all(&pages);
    }
    let Some(target) = target else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let path = if target.bytes().all(|b| b.is_ascii_digit()) {
        pages.join(&level).join(target)
    } else {
        PathBuf::from(target)
    };
    match one(&path, out) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("[decode_tile] {}: {e}", path.display());
            ExitCode::FAILURE
        }
    }
}
