// pxl-tool/fetch.rs

use std::io::Read;
use std::path::Path;

/// How many times a fetch is tried before it fails.
const TRY_CNT: u32 = 3;

/// `url`'s bytes, tried `TRY_CNT` times, a longer wait after each failure. An
/// empty body is a failure.
pub fn get(url: &str) -> Result<Vec<u8>, String> {
    let mut last = String::new();
    for k in 1..=TRY_CNT {
        match ureq::get(url).call() {
            Ok(res) => {
                let mut body = Vec::new();
                res.into_reader()
                    .read_to_end(&mut body)
                    .map_err(|e| format!("{url}: {e}"))?;
                if !body.is_empty() {
                    return Ok(body);
                }
                last = format!("{url}: an empty body");
            }
            Err(e) => last = format!("{url}: {e}"),
        }
        if k < TRY_CNT {
            std::thread::sleep(std::time::Duration::from_millis(400 * k as u64));
        }
    }
    Err(last)
}

/// `url`'s bytes, kept in `dir` under the URL's last segment: fetched once, so
/// a second run reads them from disk.
pub fn download(url: &str, dir: &Path) -> Result<Vec<u8>, String> {
    let path = dir.join(url.rsplit('/').next().ok_or("a file name")?);
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok(bytes);
    }
    let body = get(url)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(&path, &body).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(body)
}
