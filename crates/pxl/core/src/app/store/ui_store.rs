// pxl_core/app/store/ui_store.rs

//! What the kernel holds of the UI's picks: the tool, the cube, the size and
//! the decor kind placement and the hover cursor read, and the chat line the
//! bubble shows. The hotbar, its storage and the chat log are the shell's
//! (apps/web/src/app/store/ui_store.ts), which sends each pick as it is made.

use pxl_shared::spec::cell::{cube_types, DecorId, PlaceableCube};

use crate::module::world::object::size::DEFAULT_OBJECT_SIZE;
use crate::sys::global::Global;
use crate::sys::host::wall_now;

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

#[derive(Clone, Debug, PartialEq)]
pub struct UiStore {
    pub active_tool: ActiveTool,
    /// The hotbar only ever sets this to a flat colour, but the drawer picks
    /// terrain-styled cubes too.
    pub object_cube: PlaceableCube,
    pub object_size: f64,
    pub decor_id: DecorId,
    /// The last line the shell sent, which the bubble shows while it is fresh.
    pub chat_line: Option<ChatLine>,
    next_chat_line_id: u32,
}

impl UiStore {
    fn fresh() -> UiStore {
        UiStore {
            active_tool: ActiveTool::Pointer,
            object_cube: PlaceableCube::Block(cube_types::Block::red),
            object_size: DEFAULT_OBJECT_SIZE,
            decor_id: DecorId::tree,
            chat_line: None,
            next_chat_line_id: 0,
        }
    }

    pub fn select_cube(&mut self, cube: PlaceableCube) {
        self.object_cube = cube;
        self.active_tool = ActiveTool::Object;
    }

    pub fn select_decor(&mut self, id: DecorId) {
        self.decor_id = id;
        self.active_tool = ActiveTool::Decor;
    }

    pub fn append_chat_line(&mut self, text: &str) {
        let id = self.next_chat_line_id;
        self.next_chat_line_id += 1;
        self.chat_line = Some(ChatLine {
            id,
            text: text.to_string(),
            ts: wall_now(),
        });
    }
}

static UI_STORE: Global<UiStore> = Global::new();

pub fn ui_store() -> &'static mut UiStore {
    UI_STORE.get(UiStore::fresh)
}
