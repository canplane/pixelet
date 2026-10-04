// pxl-geo-bake/util.rs

use std::io::Read;
use std::path::Path;
use std::process::Command;

use xpute_core::math::scalar::round;

/// The `Content-Encoding` every baked page is uploaded with, so the transport decodes it.
pub const CONTENT_ENCODING: &str = "zstd";

#[derive(Clone, Copy, Debug)]
pub enum Level {
    /// Measured on v4 leaves: 39% under gzip -6 at four times its time; the
    /// ultra levels save a tenth more at five times again.
    Stored = 19,
    Live = 3,
}

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

pub fn pack(raw: &[u8], level: Level) -> std::io::Result<Vec<u8>> {
    zstd::bulk::compress(raw, level as i32)
}

pub fn encoding_of(stored: &[u8]) -> Option<&'static str> {
    if stored.starts_with(&ZSTD_MAGIC) {
        Some("zstd")
    } else if stored.starts_with(&GZIP_MAGIC) {
        Some("gzip")
    } else {
        None
    }
}

/// Told apart by bytes, not by a manifest: older sets are gzip.
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

/// An upload gives a directory one Content-Encoding, so a resumed bake must
/// not mix encodings.
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

pub fn git_sha() -> Option<String> {
    let out = Command::new("git").args(["rev-parse", "HEAD"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

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

/// A truncated file would otherwise count as finished on resume. The temp
/// name is per process and write, so two writers of one file cannot rename
/// each other's half-written bytes.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}-{n}.part", std::process::id()));
    let part = path.with_file_name(name);
    std::fs::write(&part, bytes)?;
    std::fs::rename(&part, path)
}

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
