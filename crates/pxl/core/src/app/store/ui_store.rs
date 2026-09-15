// pxl_core/app/store/ui_store.rs

use pxl_shared::spec::cell::{decor_ids, placeable_cube_ids, DecorId, PlaceableCubeId};

use crate::global::Global;
use crate::host::{storage_get, storage_set, wall_now};
use xpute_core::kit::json::{read_json, JsonStr};
use crate::module::world::object::size::DEFAULT_OBJECT_SIZE;

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
// decor alike, one shared MRU list. Tagged: a cube id and a decor id are both
// plain strings with no shared namespace, so telling them apart — and knowing
// whether picking one sets object_color_id or decor_id — needs the kind
// alongside the id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecentPick {
    Object(PlaceableCubeId),
    Decor(DecorId),
}

// 8, not the full catalog — the whole point of an MRU instead of a static,
// ever-growing enumeration. The strip already scrolls once it overflows a
// narrow phone's width.
const MAX_RECENT_PICKS: usize = 8;

// A pick persisted from an older session can name an id the catalog no longer
// has — dropped rather than trusted as-is, so a stale entry can't leave a dead
// id sitting in the hotbar forever.
fn is_known_pick(pick: &RecentPick) -> bool {
    match pick {
        RecentPick::Object(id) => placeable_cube_ids.iter().any(|p| p == id),
        RecentPick::Decor(id) => decor_ids.contains(id),
    }
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
    /// The full placeable catalog: the hotbar only ever sets this to a flat
    /// colour, but the drawer picks terrain-styled cubes too.
    pub object_color_id: PlaceableCubeId,
    pub object_size: f64,
    pub decor_id: DecorId,
    /// Most recent first. select_cube/select_decor are the only way to advance
    /// it: every real caller wanted the tool switch and the recency bump every
    /// single time anyway.
    pub recent_picks: Vec<RecentPick>,
    pub chat_lines: Vec<ChatLine>,
    next_chat_line_id: u32,
    pub revision: u32,
}

const STORAGE_KEY: &str = "pxl:ui_hotbar";

/// One representative per placeable genre, not the first 5 flat colors — a
/// fresh session's hotbar should hint at everything you *can* place. Filled to
/// MAX_RECENT_PICKS rather than left short: the strip's width is the eight
/// slots either way, so the empty tail read as missing rather than as room.
fn seed_picks() -> Vec<RecentPick> {
    vec![
        RecentPick::Object("white".to_string()),
        RecentPick::Object("land.BARE".to_string()),
        RecentPick::Object("water.RIVER".to_string()),
        RecentPick::Object("structure.BUILDING".to_string()),
        RecentPick::Object("transport.ROAD_DEFAULT".to_string()),
        RecentPick::Object("red".to_string()),
        RecentPick::Object("blue".to_string()),
        RecentPick::Decor(decor_ids[0]),
    ]
}

impl UiStore {
    fn fresh() -> UiStore {
        UiStore {
            active_tool: ActiveTool::Pointer,
            object_color_id: "red".to_string(),
            object_size: DEFAULT_OBJECT_SIZE,
            decor_id: "tree",
            recent_picks: seed_picks(),
            chat_lines: Vec::new(),
            next_chat_line_id: 0,
            revision: 0,
        }
    }

    fn changed(&mut self) {
        self.revision += 1;
        save(self);
    }

    pub fn set_active_tool(&mut self, tool: ActiveTool) {
        if self.active_tool != tool {
            self.active_tool = tool;
            self.revision += 1;
        }
    }

    pub fn set_object_size(&mut self, size: f64) {
        if self.object_size != size {
            self.object_size = size;
            self.revision += 1;
        }
    }

    pub fn select_cube(&mut self, id: &str) {
        self.object_color_id = id.to_string();
        self.active_tool = ActiveTool::Object;
        touch_recent(&mut self.recent_picks, RecentPick::Object(id.to_string()));
        self.changed();
    }

    pub fn select_decor(&mut self, id: DecorId) {
        self.decor_id = id;
        self.active_tool = ActiveTool::Decor;
        touch_recent(&mut self.recent_picks, RecentPick::Decor(id));
        self.changed();
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
        self.revision += 1;
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// Persisted: object_color_id/decor_id/recent_picks — real picks worth
// remembering across a reload. active_tool and chat_lines are deliberately left
// out: a fresh load should always start at the neutral pointer tool, and chat
// lines are dev-only dummy data, not user state.
fn save(s: &UiStore) {
    let picks: Vec<String> = s
        .recent_picks
        .iter()
        .map(|p| match p {
            RecentPick::Object(id) => format!("{{\"kind\":\"object\",\"id\":{}}}", json_string(id)),
            RecentPick::Decor(id) => format!("{{\"kind\":\"decor\",\"id\":{}}}", json_string(id)),
        })
        .collect();
    storage_set(
        STORAGE_KEY,
        &format!(
            "{{\"state\":{{\"object_color_id\":{},\"decor_id\":{},\"recent_picks\":[{}]}},\"version\":0}}",
            json_string(&s.object_color_id),
            json_string(s.decor_id),
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
                    let pick = if kind.eq_str("object") {
                        RecentPick::Object(id.to_string())
                    } else if kind.eq_str("decor") {
                        RecentPick::Decor(decor_id_of(id)?)
                    } else {
                        return None;
                    };
                    is_known_pick(&pick).then_some(pick)
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
    if let Some(id) = state.get("object_color_id").and_then(|v| v.as_str()) {
        let pick = RecentPick::Object(id.to_string());
        if is_known_pick(&pick) {
            s.object_color_id = id.to_string();
        }
    }
    if let Some(id) = state.get("decor_id").and_then(|v| v.as_str()).and_then(decor_id_of) {
        s.decor_id = id;
    }
    s
}

static UI_STORE: Global<UiStore> = Global::new();

/// `useUiStore.getState()`.
pub fn use_ui_store() -> &'static mut UiStore {
    UI_STORE.get(load)
}
