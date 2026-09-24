// pxl-kernel/layer/poi/detail.rs

use super::store;
use crate::{
    global::Global,
    io::{demand::Category, policy, sched::declare},
    world::change::request_world_tick,
};
use pxl_spec::poi::{
    detail::{self, Page},
    Summary,
};

struct Entry {
    summary: Summary,
    revision: u64,
    token: u64,
    offset: u64,
    page: Option<Page>,
}
#[derive(Default)]
struct Details {
    entries: Vec<Entry>,
    serial: u64,
}
static DETAILS: Global<Details> = Global::new();
fn details() -> &'static mut Details {
    DETAILS.get(Details::default)
}

pub fn select(cut: &[Summary]) {
    let d = details();
    d.entries.retain(|e| cut.contains(&e.summary) && store::store().revision(e.summary.key) == Some(e.revision));
    for summary in cut.iter().filter(|s| !s.refinable && s.count > 0).take(store::LIMITS.candidates) {
        if d.entries.iter().any(|e| e.summary.key == summary.key) {
            continue;
        }
        let Some(revision) = store::store().revision(summary.key) else { continue };
        d.serial += 1;
        d.entries.push(Entry {
            summary: *summary,
            revision,
            token: d.serial,
            offset: 0,
            page: None,
        });
    }
    demand();
}

fn demand() {
    let wants: Vec<_> = details().entries.iter().filter(|e| e.page.is_none()).map(|e| (e.token, policy::poi(0.0))).collect();
    declare(Category::PoiDetail, &wants);
}
pub fn name(token: u64) -> String {
    details().entries.iter().find(|e| e.token == token).map(|e| detail::name(e.summary.key, e.offset)).unwrap_or_default()
}

pub fn adopt(token: u64, bytes: &[u8]) -> Result<(), &'static str> {
    let Some(e) = details().entries.iter_mut().find(|e| e.token == token) else { return Ok(()) };
    let page = detail::decode(bytes)?;
    if page.key != e.summary.key || page.offset != e.offset {
        return Err("detail coverage mismatch");
    }
    if page.revision != e.revision {
        store::store().invalidate(page.key);
        request_world_tick();
        return Err("detail snapshot changed");
    }
    if page.total != e.summary.count {
        return Err("detail count mismatch");
    }
    if page
        .records
        .iter()
        .any(|r| (0..3).any(|i| r.position[i] < e.summary.bounds[i] || r.position[i] > e.summary.bounds[i + 3]))
    {
        return Err("detail outside summary");
    }
    e.page = Some(page);
    demand();
    request_world_tick();
    Ok(())
}

pub fn label(summary: &Summary) -> String {
    if summary.count != 1 {
        return summary.count.to_string();
    }
    let name = details()
        .entries
        .iter()
        .find(|e| e.summary == *summary)
        .and_then(|e| e.page.as_ref())
        .and_then(|p| p.records.first())
        .map(|r| if r.text.is_empty() { r.id.as_str() } else { r.text.as_str() });
    let Some(name) = name else { return "…".into() };
    // The original string remains in the detail backing for the DOM.
    let mut label: String = name.chars().take(48).collect();
    if name.chars().count() > 48 {
        label.push('…');
    }
    label
}

pub fn page(key: u64, next: bool) {
    let d = details();
    let Some(e) = d.entries.iter_mut().find(|e| e.summary.key == key) else { return };
    let offset = if next {
        let Some(n) = e.page.as_ref().and_then(Page::next) else { return };
        n
    } else {
        0
    };
    if offset == e.offset {
        return;
    }
    d.serial += 1;
    e.token = d.serial;
    e.offset = offset;
    e.page = None;
    demand();
    request_world_tick();
}

pub fn snapshot<W: std::fmt::Write>(out: &mut W) -> std::fmt::Result {
    use xpute_core::codec::json::Quoted;
    out.write_str("[")?;
    for (i, e) in details().entries.iter().enumerate() {
        if i != 0 {
            out.write_str(",")?;
        }
        write!(
            out,
            "{{\"key\":{},\"count\":{},\"offset\":{},\"next\":{},\"records\":[",
            Quoted(e.summary.key.to_string()),
            Quoted(e.summary.count.to_string()),
            Quoted(e.offset.to_string()),
            e.page.as_ref().and_then(Page::next).is_some()
        )?;
        if let Some(page) = &e.page {
            for (j, record) in page.records.iter().enumerate() {
                if j != 0 {
                    out.write_str(",")?;
                }
                write!(out, "{{\"id\":{},\"text\":{}}}", Quoted(&record.id), Quoted(&record.text))?;
            }
        }
        out.write_str("]}")?;
    }
    out.write_str("]")
}
