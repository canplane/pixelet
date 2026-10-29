// pxl-tool/fetch.rs

use std::io::Read;
use std::path::Path;

/// `url`'s bytes, kept in `dir` under the URL's last segment: fetched once, so
/// a second run reads them from disk.
pub fn fetch_kept(url: &str, dir: &Path) -> Result<Vec<u8>, String> {
    let path = dir.join(url.rsplit('/').next().ok_or("a file name")?);
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok(bytes);
    }
    let mut body = Vec::new();
    ureq::get(url)
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(&path, &body).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(body)
}
