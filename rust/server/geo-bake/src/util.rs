// pxl-geo-bake/util.rs

use std::path::Path;
use std::process::Command;

use pxl_spec::dirs;
use xpute_kit::math::scalar::round;
use zstd::bulk;
use zstd::stream;

/// The `Content-Encoding` every baked tile is uploaded with, so the transport decodes it.
pub const CONTENT_ENCODING: &str = "zstd";

/// FNV-1a 64.
pub const fn fnv1a(bytes: &[u8]) -> u64 {
    let (mut h, mut i) = (0xcbf2_9ce4_8422_2325u64, 0);
    while i < bytes.len() {
        h = (h ^ bytes[i] as u64).wrapping_mul(0x0100_0000_01b3);
        i += 1;
    }
    h
}

#[derive(Clone, Copy, Debug)]
pub enum Level {
    /// Measured on v4 leaves: 39% under gzip -6 at four times its time; the
    /// ultra levels save a tenth more at five times again.
    Stored = 19,
    Live = 3,
}

pub fn pack(raw: &[u8], level: Level) -> std::io::Result<Vec<u8>> {
    bulk::compress(raw, level as i32)
}

pub fn unpack(stored: &[u8]) -> std::io::Result<Vec<u8>> {
    stream::decode_all(stored)
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

/// HEAD, and where the working tree differs, `+` its tree (untracked files
/// too): a bake from uncommitted code names that code. Read when a bake
/// starts, which is when its binary was built: commits made while it runs
/// are not what it ran.
pub fn git_sha() -> Option<String> {
    let idx = dirs::temp_dir(&format!("bake-index-{}", std::process::id()));
    std::fs::create_dir_all(idx.parent()?).ok()?;
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .env("GIT_INDEX_FILE", &idx)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let head = git(&["rev-parse", "HEAD"])?;
    let tree = git(&["add", "-A"]).and_then(|_| git(&["write-tree"]));
    let _ = std::fs::remove_file(&idx);
    match tree {
        Some(t) if git(&["rev-parse", "HEAD^{tree}"]).as_deref() != Some(t.as_str()) => {
            Some(format!("{head}+{t}"))
        }
        _ => Some(head),
    }
}

pub fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs();
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{}T{h:02}:{m:02}:{s:02}.000Z", day_of(secs / 86_400))
}

/// The civil date of a day count from the epoch, Howard Hinnant's algorithm.
fn day_of(days: u64) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);

    format!("{y:04}-{mo:02}-{d:02}")
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

/// A set's manifest at `path`: `fields`, and what every set says of itself;
/// `built` is the `git_sha` the bake started at.
pub fn write_manifest(
    path: &Path,
    built: Option<&str>,
    fields: serde_json::Value,
) -> Result<(), String> {
    let mut manifest = serde_json::json!({
        "updated_at": now_iso8601(),
        "git_sha": built,
        "content_encoding": CONTENT_ENCODING,
        "content_type": "application/octet-stream",
    });
    if let (Some(out), serde_json::Value::Object(fields)) = (manifest.as_object_mut(), fields) {
        out.extend(fields);
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("[bake] manifest -> {}", path.display());
    Ok(())
}
