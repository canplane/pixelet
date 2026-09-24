// pxl-spec/poi/wire.rs

use super::{xy, Node, Summary};

const MAGIC: &[u8; 4] = b"PXS1";
const SUMMARY_BYTES: usize = 8 + 8 + 48 + 1;
pub const MAX_PACKET_BYTES: usize = 16 + SUMMARY_BYTES * (1 + xy::CHILDREN);

fn valid(s: &Summary) -> bool {
    let Some(_) = xy::depth(s.key) else { return false };
    if !s.bounds.iter().all(|v| v.is_finite()) {
        return false;
    }
    let b = xy::bounds(s.key);
    (0..3).all(|i| s.bounds[i] <= s.bounds[i + 3]) && s.bounds[0] >= b[0] && s.bounds[1] >= b[1] && s.bounds[3] < b[2] && s.bounds[4] < b[3]
}

pub fn validate(node: &Node) -> Result<(), &'static str> {
    if node.revision == 0 || !valid(&node.summary) || node.children.len() > xy::CHILDREN || node.summary.refinable == node.children.is_empty() {
        return Err("invalid summary node");
    }
    let mut count = 0u64;
    let mut seen = Vec::new();
    for c in &node.children {
        if !valid(c)
            || c.count == 0
            || c.key / 4 != node.summary.key
            || seen.contains(&c.key)
            || !(0..3).all(|i| c.bounds[i] >= node.summary.bounds[i] && c.bounds[i + 3] <= node.summary.bounds[i + 3])
        {
            return Err("invalid refinement coverage");
        }
        seen.push(c.key);
        count = count.checked_add(c.count).ok_or("count overflow")?;
    }
    if !node.children.is_empty() && count != node.summary.count {
        return Err("refinement count mismatch");
    }
    Ok(())
}

pub fn encode(node: &Node) -> Result<Vec<u8>, &'static str> {
    validate(node)?;
    let mut out = Vec::with_capacity(MAX_PACKET_BYTES);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&node.revision.to_le_bytes());
    out.extend_from_slice(&(node.children.len() as u32).to_le_bytes());
    for s in std::iter::once(&node.summary).chain(&node.children) {
        out.extend_from_slice(&s.key.to_le_bytes());
        out.extend_from_slice(&s.count.to_le_bytes());
        for v in s.bounds {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.push(u8::from(s.refinable));
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Node, &'static str> {
    if bytes.len() < 16 + SUMMARY_BYTES || bytes.len() > MAX_PACKET_BYTES || &bytes[..4] != MAGIC {
        return Err("invalid summary packet");
    }
    let word = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
    let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if n > xy::CHILDREN || bytes.len() != 16 + SUMMARY_BYTES * (n + 1) {
        return Err("invalid summary length");
    }
    let read_summary = |i| -> Result<Summary, &'static str> {
        let at = 16 + i * SUMMARY_BYTES;
        let flag = bytes[at + 64];
        if flag > 1 {
            return Err("invalid refinement flag");
        }
        Ok(Summary {
            key: word(at),
            count: word(at + 8),
            bounds: std::array::from_fn(|j| f64::from_bits(word(at + 16 + j * 8))),
            refinable: flag != 0,
        })
    };
    let node = Node {
        revision: word(4),
        summary: read_summary(0)?,
        children: (1..=n).map(read_summary).collect::<Result<_, _>>()?,
    };
    validate(&node)?;
    Ok(node)
}

#[cfg(test)]
#[path = "wire.test.rs"]
mod test;
