// pxl-geo-bake/poi/detail.rs

use crate::util::{pack, write_atomic, Level};
use pxl_spec::poi::{
    detail::{self, Page},
    xy, Entity, Node,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub fn write(backing: &Path, tiles: &Path, nodes: &[Node], packing: Level) -> Result<(), String> {
    let terminals: BTreeMap<_, _> = nodes.iter().filter(|n| !n.summary.refinable).map(|n| (n.summary.key, n)).collect();
    let mut pages: BTreeMap<u64, Page> = BTreeMap::new();
    let mut sizes: BTreeMap<u64, usize> = BTreeMap::new();
    for line in BufReader::new(File::open(backing).map_err(|e| e.to_string())?).lines() {
        let entity: Entity = serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut key = xy::key_at(entity.position, xy::MAX_DEPTH).ok_or("invalid detail position")?;
        while !terminals.contains_key(&key) {
            key >>= 2;
            if key == 0 {
                return Err("detail without terminal summary".into());
            }
        }
        let node = terminals[&key];
        let size = detail::record(&entity)?.len();
        let page = pages.entry(key).or_insert_with(|| Page {
            revision: node.revision,
            key,
            offset: 0,
            total: node.summary.count,
            records: Vec::new(),
        });
        let bytes = sizes.entry(key).or_insert(detail::HEADER_BYTES);
        if *bytes + size > detail::MAX_PACKET_BYTES || page.records.len() == detail::PAGE_RECORDS {
            flush(tiles, page, packing)?;
            page.offset += page.records.len() as u64;
            page.records.clear();
            *bytes = detail::HEADER_BYTES;
        }
        page.records.push(entity);
        *bytes += size;
        if page.offset + page.records.len() as u64 == page.total {
            flush(tiles, page, packing)?;
            pages.remove(&key);
            sizes.remove(&key);
        }
    }
    if !pages.is_empty() {
        return Err("incomplete canonical detail backing".into());
    }
    Ok(())
}

fn flush(tiles: &Path, page: &Page, packing: Level) -> Result<(), String> {
    let raw = detail::encode(page)?;
    let packed = pack(&raw, packing).map_err(|e| e.to_string())?;
    write_atomic(&tiles.join(detail::name(page.key, page.offset)), &packed).map_err(|e| e.to_string())
}
