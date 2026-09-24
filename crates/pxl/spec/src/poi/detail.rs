// pxl-spec/poi/detail.rs

use super::{xy, Entity};

pub const PAGE_RECORDS: usize = 16;
pub const MAX_PACKET_BYTES: usize = 64 * 1024;
pub const HEADER_BYTES: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub revision: u64,
    pub key: u64,
    pub offset: u64,
    pub total: u64,
    pub records: Vec<Entity>,
}
impl Page {
    pub fn next(&self) -> Option<u64> {
        let next = self.offset.checked_add(self.records.len() as u64)?;
        (next < self.total).then_some(next)
    }
}

pub fn name(key: u64, offset: u64) -> String {
    format!("d{}-{offset}", xy::name(key))
}
pub fn parse(name: &str) -> Option<(u64, u64)> {
    let (key, offset) = name.strip_prefix('d')?.split_once('-')?;
    if offset.is_empty() || !offset.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((xy::parse(key)?, offset.parse().ok()?))
}

pub fn record(entity: &Entity) -> Result<Vec<u8>, &'static str> {
    if entity.id.is_empty() || !entity.position.iter().all(|v| v.is_finite()) {
        return Err("invalid detail entity");
    }
    let size = 32usize.checked_add(entity.id.len()).and_then(|n| n.checked_add(entity.text.len())).ok_or("detail record size")?;
    if size + HEADER_BYTES > MAX_PACKET_BYTES {
        return Err("entity text exceeds detail packet budget");
    }
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&(entity.id.len() as u32).to_le_bytes());
    out.extend_from_slice(&(entity.text.len() as u32).to_le_bytes());
    for v in entity.position {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(entity.id.as_bytes());
    out.extend_from_slice(entity.text.as_bytes());
    Ok(out)
}

pub fn encode(page: &Page) -> Result<Vec<u8>, &'static str> {
    if page.revision == 0
        || xy::depth(page.key).is_none()
        || page.records.len() > PAGE_RECORDS
        || page.offset.checked_add(page.records.len() as u64).is_none_or(|n| n > page.total)
        || (page.records.is_empty() && page.offset < page.total)
    {
        return Err("invalid detail page");
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"PXD1");
    for n in [page.revision, page.key, page.offset, page.total] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    out.extend_from_slice(&(page.records.len() as u32).to_le_bytes());
    for e in &page.records {
        out.extend_from_slice(&record(e)?);
    }
    if out.len() > MAX_PACKET_BYTES {
        return Err("detail packet budget");
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Page, &'static str> {
    if bytes.len() < HEADER_BYTES || bytes.len() > MAX_PACKET_BYTES || &bytes[..4] != b"PXD1" {
        return Err("invalid detail packet");
    }
    let word = |i| u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
    let n = u32::from_le_bytes(bytes[36..40].try_into().unwrap()) as usize;
    if n > PAGE_RECORDS {
        return Err("detail record budget");
    }
    let mut page = Page {
        revision: word(4),
        key: word(12),
        offset: word(20),
        total: word(28),
        records: Vec::new(),
    };
    let mut at = HEADER_BYTES;
    for _ in 0..n {
        let header = bytes.get(at..at + 32).ok_or("short detail record")?;
        let id = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
        let text = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let position = std::array::from_fn(|i| f64::from_le_bytes(header[8 + i * 8..16 + i * 8].try_into().unwrap()));
        at += 32;
        let read = |a: usize, n: usize| -> Result<String, &'static str> {
            let end = a.checked_add(n).ok_or("detail length overflow")?;
            std::str::from_utf8(bytes.get(a..end).ok_or("short detail text")?)
                .map(str::to_owned)
                .map_err(|_| "invalid detail UTF-8")
        };
        let id_value = read(at, id)?;
        at += id;
        let text_value = read(at, text)?;
        at += text;
        page.records.push(Entity {
            id: id_value,
            text: text_value,
            position,
        });
    }
    if at != bytes.len() {
        return Err("trailing detail bytes");
    }
    encode(&page)?;
    Ok(page)
}

#[cfg(test)]
#[path = "detail.test.rs"]
mod test;
