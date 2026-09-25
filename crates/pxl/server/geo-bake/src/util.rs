// pxl-geo-bake/util.rs

//! What every bake tool needs and none of them owns.

use std::io::Read;
use std::path::Path;
use std::process::Command;

use xpute_core::math::scalar::round;

/// How every baked page is stored and served: the `Content-Encoding` its
/// upload carries, so a browser or an app's HTTP stack decodes it on the way
/// in and nothing past the transport sees the codec
/// (issue/20260913-compression-by-traffic.md).
pub const CONTENT_ENCODING: &str = "zstd";

/// How hard a page is packed. The level is the transport's business, like
/// the codec: what a page holds is its packet, and two packings of one packet
/// are the same page.
#[derive(Clone, Copy, Debug)]
pub enum Level {
    /// What a bake stores: the top of zstd's regular levels, the usual choice
    /// for a file compressed once and fetched many times. Measured on v4's
    /// leaves: 39% under gzip -6, four times its compression time, the same
    /// decode. The ultra levels take another tenth off at five times the
    /// time again, which every leaf of a bake would pay.
    Stored = 19,
    /// What is packed while someone waits for it — the dev tile server's
    /// pages — at zstd's own default.
    Live = 3,
}

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// A page's raw packet, packed.
pub fn pack(raw: &[u8], level: Level) -> std::io::Result<Vec<u8>> {
    zstd::bulk::compress(raw, level as i32)
}

/// The encoding a stored object is in, by its own first bytes.
pub fn encoding_of(stored: &[u8]) -> Option<&'static str> {
    if stored.starts_with(&ZSTD_MAGIC) {
        Some("zstd")
    } else if stored.starts_with(&GZIP_MAGIC) {
        Some("gzip")
    } else {
        None
    }
}

/// A stored object back to its raw packet, in whichever encoding it was
/// stored: a tile set baked before zstd is gzip, and it is told apart by its
/// bytes rather than by a manifest that may describe another run.
pub fn unpack(stored: &[u8]) -> std::io::Result<Vec<u8>> {
    match encoding_of(stored) {
        Some("zstd") => zstd::stream::decode_all(stored),
        Some(_) => {
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(stored).read_to_end(&mut raw)?;
            Ok(raw)
        }
        None => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "neither zstd nor gzip")),
    }
}

/// Refuses to add objects to a directory whose objects are in another
/// encoding. An upload gives a whole directory one Content-Encoding, so a
/// resumed bake that skipped the old objects and wrote new ones beside them
/// would publish half of them wrongly labeled.
pub fn refuse_mixed(dir: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(()) };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() || name.ends_with(".part") || name.ends_with(".empty") {
            continue;
        }
        let mut head = [0u8; 4];
        let n = std::fs::File::open(entry.path()).and_then(|mut f| f.read(&mut head)).map_err(|e| e.to_string())?;
        return match encoding_of(&head[..n]) {
            Some(CONTENT_ENCODING) => Ok(()),
            other => Err(format!(
                "{} holds {} objects and this bake writes {CONTENT_ENCODING}: bake into a fresh directory, or pass --force to rewrite every object",
                dir.display(),
                other.unwrap_or("unrecognized")
            )),
        };
    }
    Ok(())
}

pub fn fmt_bytes(n: f64) -> String {
    if n < 1024.0 * 1024.0 {
        format!("{:.1} kB", n / 1024.0)
    } else if n < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} MB", n / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", n / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn fmt_dur(ms: f64) -> String {
    let s = round(ms / 1000.0) as u64;
    let (h, m) = (s / 3600, (s % 3600) / 60);
    if h > 0 {
        format!("{h}h{m}m")
    } else {
        format!("{m}m{}s", s % 60)
    }
}

/// The commit that produced a published set — the other half of "what is v2":
/// a catalog says which *data* it was baked from, this says which *code* did
/// the baking. None rather than an error outside a git checkout: a missing sha
/// in a manifest is a visible gap, not a reason to fail the bake.
pub fn git_sha() -> Option<String> {
    let out = Command::new("git").args(["rev-parse", "HEAD"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `new Date().toISOString()`, which is what the manifests have always carried.
/// Written out rather than taken from a date crate: one timestamp, one format,
/// and no dependency that would then be in the workspace for everything else.
pub fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("after the epoch").as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Civil date from a day count, Howard Hinnant's algorithm.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);

    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}.000Z")
}

/// Writes to a temp name and renames. A crash mid-write would otherwise leave
/// a truncated file that the resume path — the file existing — treats as a
/// finished object forever after.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let part = path.with_extension("part");
    std::fs::write(&part, bytes)?;
    std::fs::rename(&part, path)
}

/// Counts and sizes every finished object in a directory, ignoring the `.part`
/// files an interrupted write leaves and the `.empty` markers that are local
/// to a resume and never uploaded.
pub fn measure_output(dir: &Path) -> std::io::Result<(u64, u64)> {
    let (mut count, mut bytes) = (0u64, 0u64);
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !entry.file_type()?.is_file() || name.ends_with(".part") || name.ends_with(".empty") {
            continue;
        }
        count += 1;
        bytes += entry.metadata()?.len();
    }
    Ok((count, bytes))
}

#[cfg(test)]
#[path = "util.test.rs"]
mod test;
