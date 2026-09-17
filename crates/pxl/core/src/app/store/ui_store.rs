// pxl_core/app/store/ui_store.rs

use pxl_shared::spec::cell::{cube_types, decor_ids, placeable_cube_id, placeable_cubes, DecorId, PlaceableCube};
use xpute_core::kit::json::{read_json, JsonStr, Quoted};

use crate::module::world::object::size::DEFAULT_OBJECT_SIZE;
use crate::sys::global::Global;
use crate::sys::host::{storage_get, storage_set, wall_now};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActiveTool {
    #[default]
    Pointer,
    Object,
    Decor,
    Eraser,
}

// ts (wall clock at append) lets the chat bubble derive its own remaining show
// time from wall-clock, not from when it happened to start showing — without
// it, a character swap resets its state and it has no way to tell an old,
// already-dismissed line apart from a brand new one.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatLine {
    pub id: u32,
    pub text: String,
    pub ts: f64,
}

const MAX_CHAT_LINES: usize = 10;

// The hotbar: "whatever you actually reached for lately," across cube and
// decor alike, one shared MRU list. Tagged: a cube and a decor kind are two
// catalogs — telling them apart, and knowing whether picking one sets
// object_cube or decor_id, needs the kind alongside.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecentPick {
    Object(PlaceableCube),
    Decor(DecorId),
}

// 8, not the full catalog — the whole point of an MRU instead of a static,
// ever-growing enumeration. The strip already scrolls once it overflows a
// narrow phone's width.
const MAX_RECENT_PICKS: usize = 8;

/// The cube an id names, or None where the catalog has no such id — a pick
/// persisted from an older session, dropped rather than trusted as-is so a
/// stale entry cannot leave a dead swatch sitting in the hotbar forever.
/// Reading storage is the one place the kernel still takes a cube by its id:
/// a browser's own blob is words, written before the numbering it would
/// otherwise carry.
fn cube_of(id: &str) -> Option<PlaceableCube> {
    placeable_cubes.iter().copied().find(|cube| placeable_cube_id(*cube) == id)
}

// Only ever inserts a genuinely new pick at the front — re-picking one already
// in the strip leaves the list untouched, so tapping an existing hotbar slot
// doesn't reshuffle the ones around it (confirmed live as disorienting).
fn touch_recent(recent_picks: &mut Vec<RecentPick>, pick: RecentPick) {
    if recent_picks.contains(&pick) {
        return;
    }
    recent_picks.insert(0, pick);
    recent_picks.truncate(MAX_RECENT_PICKS);
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiStore {
    pub active_tool: ActiveTool,
    /// The hotbar only ever sets this to a flat colour, but the drawer picks
    /// terrain-styled cubes too.
    pub object_cube: PlaceableCube,
    pub object_size: f64,
    pub decor_id: DecorId,
    /// Most recent first. select_cube/select_decor are the only way to advance
    /// it: every real caller wanted the tool switch and the recency bump every
    /// single time anyway.
    pub recent_picks: Vec<RecentPick>,
    pub chat_lines: Vec<ChatLine>,
    next_chat_line_id: u32,
}

const STORAGE_KEY: &str = "pxl:ui_hotbar";

/// One representative per placeable genre, not the first 5 flat colors — a
/// fresh session's hotbar should hint at everything you *can* place. Filled to
/// MAX_RECENT_PICKS rather than left short: the strip's width is the eight
/// slots either way, so the empty tail read as missing rather than as room.
fn seed_picks() -> Vec<RecentPick> {
    vec![
        RecentPick::Object(PlaceableCube::Block(cube_types::Block::white)),
        RecentPick::Object(PlaceableCube::Land(cube_types::Land::bare)),
        RecentPick::Object(PlaceableCube::Water(cube_types::Water::river)),
        RecentPick::Object(PlaceableCube::Structure(cube_types::Structure::building)),
        RecentPick::Object(PlaceableCube::Transport(cube_types::Transport::road_default)),
        RecentPick::Object(PlaceableCube::Block(cube_types::Block::red)),
        RecentPick::Object(PlaceableCube::Block(cube_types::Block::blue)),
        RecentPick::Decor(decor_ids[0]),
    ]
}

impl UiStore {
    fn fresh() -> UiStore {
        UiStore {
            active_tool: ActiveTool::Pointer,
            object_cube: PlaceableCube::Block(cube_types::Block::red),
            object_size: DEFAULT_OBJECT_SIZE,
            decor_id: "tree",
            recent_picks: seed_picks(),
            chat_lines: Vec::new(),
            next_chat_line_id: 0,
        }
    }

    pub fn select_cube(&mut self, cube: PlaceableCube) {
        self.object_cube = cube;
        self.active_tool = ActiveTool::Object;
        touch_recent(&mut self.recent_picks, RecentPick::Object(cube));
        save(self);
    }

    pub fn select_decor(&mut self, id: DecorId) {
        self.decor_id = id;
        self.active_tool = ActiveTool::Decor;
        touch_recent(&mut self.recent_picks, RecentPick::Decor(id));
        save(self);
    }

    pub fn append_chat_line(&mut self, text: &str) {
        let id = self.next_chat_line_id;
        self.next_chat_line_id += 1;
        self.chat_lines.push(ChatLine {
            id,
            text: text.to_string(),
            ts: wall_now(),
        });
        let excess = self.chat_lines.len().saturating_sub(MAX_CHAT_LINES);
        self.chat_lines.drain(..excess);
    }
}

// Persisted: object_cube_id/decor_id/recent_picks — real picks worth
// remembering across a reload. active_tool and chat_lines are deliberately left
// out: a fresh load should always start at the neutral pointer tool, and chat
// lines are dev-only dummy data, not user state.
//
// A cube is stored under the id the catalog spells it out with, not the
// number it is held as: a browser's storage outlives the numbering, which is
// assignment order over a catalog that grows.
//
// Version 1 is that id gaining its channel ("block.red", not "red"): a blob
// written before it resolves to no cube and falls back to the seed, which is
// what a pick the catalog no longer has does anyway.
fn save(s: &UiStore) {
    let picks: Vec<String> = s
        .recent_picks
        .iter()
        .map(|p| match p {
            RecentPick::Object(cube) => format!("{{\"kind\":\"object\",\"id\":{}}}", Quoted(placeable_cube_id(*cube))),
            RecentPick::Decor(id) => format!("{{\"kind\":\"decor\",\"id\":{}}}", Quoted(id)),
        })
        .collect();
    storage_set(
        STORAGE_KEY,
        &format!(
            "{{\"state\":{{\"object_cube_id\":{},\"decor_id\":{},\"recent_picks\":[{}]}},\"version\":1}}",
            Quoted(placeable_cube_id(s.object_cube)),
            Quoted(s.decor_id),
            picks.join(",")
        ),
    );
}

fn decor_id_of(id: JsonStr) -> Option<DecorId> {
    decor_ids.iter().copied().find(|d| id.eq_str(d))
}

// A blind merge is fine for always-valid fields, not here: a persisted id
// might no longer exist, and an empty persisted recent_picks (every entry
// filtered out) should fall back to the fresh default rather than leaving the
// hotbar empty.
fn load() -> UiStore {
    let mut s = UiStore::fresh();
    let Some(raw) = storage_get(STORAGE_KEY) else { return s };
    let Some(state) = read_json(&raw).and_then(|j| j.get("state")) else { return s };
    let mut picks: Vec<RecentPick> = state
        .get("recent_picks")
        .map(|arr| {
            arr.items()
                .filter_map(|p| {
                    let (id, kind) = (p.get("id")?.as_str()?, p.get("kind")?.as_str()?);
                    // Both arms resolve through the catalog, so a pick it no
                    // longer has drops here.
                    if kind.eq_str("object") {
                        Some(RecentPick::Object(cube_of(&id.to_string())?))
                    } else if kind.eq_str("decor") {
                        Some(RecentPick::Decor(decor_id_of(id)?))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    // A short list is padded from the seed rather than left short: the padding
    // is the first thing to fall off the end once a real pick arrives, since
    // touch_recent prepends and truncates.
    for seed in seed_picks() {
        if picks.len() >= MAX_RECENT_PICKS {
            break;
        }
        if !picks.contains(&seed) {
            picks.push(seed);
        }
    }
    s.recent_picks = picks;
    if let Some(cube) = state.get("object_cube_id").and_then(|v| v.as_str()).and_then(|id| cube_of(&id.to_string())) {
        s.object_cube = cube;
    }
    if let Some(id) = state.get("decor_id").and_then(|v| v.as_str()).and_then(decor_id_of) {
        s.decor_id = id;
    }
    s
}

static UI_STORE: Global<UiStore> = Global::new();

pub fn ui_store() -> &'static mut UiStore {
    UI_STORE.get(load)
}
