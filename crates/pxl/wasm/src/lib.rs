//! The one WebAssembly module: pxl-core behind a boot block and a doorbell.
//!
//! No wasm-bindgen. The central scheduler gives the module its memory, and
//! the module lays out everything inside it when it is booted (`boot`,
//! pxl_core::mem); the shell reads the boot block once, and everything after
//! crosses in the rings in that memory: a command and its packet in, its
//! reply and the kernel's signals out, and `interrupt` to hand over the turn.
//!
//! The module is single-threaded, so the state is a plain global.

mod executor;

use pxl_core::mem;
use pxl_core::render::layer::pipeline::volume::Volumes;
use pxl_core::sys::global::Global;
use pxl_core::sys::kernel::kernel;
use pxl_shared::spec::cmd::Cmd;
use xpute_core::io::marshal::tree::{BranchCursor, PacketWriter, TreeReader};
use xpute_core::ipc::frame::FrameFlag;
use xpute_core::ipc::ring::{ring_bytes, slots_bytes, Ring};
use xpute_core::kit::array::{ArrayBuffer, U8Array};
use xpute_core::sys::errno::Errno;
use xpute_core::sys::error::MarshalError;
use xpute_core::sys::mem::{write_section, SECTION_WORDS};

extern "C" {
    /// Where the linker stopped: the end of `.rodata`, `.data` and `.bss`, and
    /// so where the kernel may begin placing its own ranges. wasm-ld defines
    /// it, and `--global-base` (build.rs) is what decides where the data it
    /// measures began.
    ///
    /// Not `__heap_base`, though that symbol sits eight bytes further on and
    /// would do. The linker calls it that because a C runtime starts its
    /// `sbrk` there; this kernel's heap is past the fixed ranges and past the
    /// terrain, so borrowing the name would leave `heap` meaning two things
    /// four megabytes apart.
    #[allow(non_upper_case_globals)]
    static __data_end: u8;
    /// The top of the stack: `-z stack-size` past its start, which
    /// `--stack-first` puts at the bottom of the memory. wasm-ld defines it.
    #[allow(non_upper_case_globals)]
    static __stack_high: u8;
}

/// Where the linker stopped, as an address.
fn data_end() -> u32 {
    core::ptr::addr_of!(__data_end) as u32
}

/// The heap: from the page past the last fixed range to the end of the memory,
/// ascending. Everything before it is settled before it — the stack, the
/// rings, the linker's data, and `.fixed` — so the heap is what is left, and
/// what is left is whatever the device gave.
struct ModuleHeap;

/// The largest block the heap's tree can describe. Two gigabytes is what a
/// 32-bit address space holds a whole power of two of, so it is the top of the
/// shape and not a claim about any device: the tree begins as a single leaf and
/// doubles only as far as a request makes it, and how much of it is real comes
/// from `end` at runtime.
const HEAP_MAX_LOG2: usize = 31;

/// The smallest block, which is the page the rest of the memory is counted in
/// (mem.json's PAGE_BYTES).
const HEAP_MIN_LOG2: usize = memory_shift(mem::PAGE_BYTES);

const fn memory_shift(bytes: u32) -> usize {
    assert!(bytes.is_power_of_two());
    bytes.trailing_zeros() as usize
}

/// A bit for every node of the tree that can be a parent. The exponents give
/// the length and the compiler will not derive it for us, so it is written
/// once here and `BuddyMalloc` checks it against them on the first request.
const HEAP_SPLIT_BYTES: usize = (1 << (HEAP_MAX_LOG2 - HEAP_MIN_LOG2)) / 8;

impl xpute_core::ds::buddy_tree::Range for ModuleHeap {
    const MIN_LOG2: usize = HEAP_MIN_LOG2;
    const MAX_LOG2: usize = HEAP_MAX_LOG2;
    type Split = [u8; HEAP_SPLIT_BYTES];
    const SPLIT_INIT: Self::Split = [0; HEAP_SPLIT_BYTES];

    fn base() -> usize {
        // A pure function of the linker's answer and the map's constants, with
        // no state and no order to be in: the first block is asked for long
        // before anything has laid the memory out.
        mem::heap_at(data_end()) as usize
    }
    fn end() -> usize {
        core::arch::wasm32::memory_size(0) * (1 << 16)
    }
}

/// The module's one allocator: a slab of size classes over a buddy of 4 KiB
/// pages, both over the range above. The buddy is what the sections' blocks
/// come from; the slab is what a `Vec` or a `String` comes from, and it
/// borrows and returns whole runs of pages to the same buddy, so neither level
/// can starve while the other has room.
#[global_allocator]
static ALLOC: xpute_core::ds::slab::SlabMalloc<ModuleHeap> = xpute_core::ds::slab::SlabMalloc::new();

struct State {
    /// The submission ring (shell → core) and the completion ring (core →
    /// shell), in blocks of the fixed section taken once. Where in that section
    /// they sit is the section's to answer, and the boot block carries the
    /// answer out to the shell.
    sq: Ring,
    /// The completion ring and its spill.
    outbox: Outbox,
    /// The UI snapshot last posted, by hash: one is posted only when it changed.
    snapshot: u64,
    /// The block `boot` hands out.
    boot: [u32; BOOT_WORDS],
}

// The front of the memory: where the rings are and where the linker was told
// its own data may begin. Written by build.rs, which emits the same numbers
// as `--global-base`, so the two cannot disagree.
include!(concat!(env!("OUT_DIR"), "/ring_layout.rs"));

/// The three rings, in the order their descriptors and frames are laid out.
const SQ: u32 = 0;
const CQ: u32 = 1;
const SPILL: u32 = 2;

const _: () = assert!(ring_bytes(RING_CAPACITY) <= RING_DESC_STRIDE, "a ring's descriptors outgrow their stride");
const _: () = assert!(slots_bytes(RING_CAPACITY, RING_SLOT_BYTES) == RING_SLOT_STRIDE, "a ring's payloads are not its stride");
const _: () = assert!(RING_SLOT_BYTES == mem::PAGE_BYTES, "a slot's payload is the page the rest of the memory is counted in");

/// The whole memory as one buffer, which is what a ring's two areas are cut
/// from: the descriptors near the bottom and the slots' payloads past them, at
/// addresses build.rs fixed rather than anyone hands out.
///
/// # Safety
/// The module's own memory, addressed from 0.
unsafe fn module_memory() -> ArrayBuffer {
    unsafe { ArrayBuffer::from_raw(core::ptr::null_mut(), core::arch::wasm32::memory_size(0) * (1 << 16)) }
}

/// The nth ring, laid out empty.
fn new_ring(n: u32) -> Ring {
    // SAFETY: the module's own memory; the two ranges are build.rs's and no
    // one else's, the linker included.
    let mem = unsafe { module_memory() };
    Ring::init(mem, RING_DESC_AT + n * RING_DESC_STRIDE, RING_CAPACITY, RING_SLOT_AT + n * RING_SLOT_STRIDE, RING_SLOT_BYTES)
}

/// Where a ring's words start in the memory.
fn ring_address(ring: &Ring) -> u32 {
    ring.words.as_slice().as_ptr() as u32
}

/// The module's one state, made on first use. xpute's device for it rather
/// than a `static mut`, whose references the compiler now warns of: the module
/// is single-threaded and no export re-enters another, which is the promise
/// `Global` is built on.
static STATE: Global<State> = Global::new();

extern "C" {
    /// The client's: told a panic's message before the trap, since the trap
    /// itself carries none — `panic = "abort"` and no hook would leave only
    /// "unreachable" in the console.
    #[link_name = "panic"]
    fn host_panic(ptr: *const u8, len: usize);
    /// The client's clock, in milliseconds: what a tick's budget is spent against.
    #[link_name = "now"]
    fn host_now() -> f64;
}

fn state() -> &'static mut State {
    STATE.get(|| {
        std::panic::set_hook(Box::new(|info| {
            let text = info.to_string();
            // SAFETY: the client's import, told a message it copies out before
            // the trap.
            unsafe { host_panic(text.as_ptr(), text.len()) };
        }));
        mem::map().unwrap_or_else(lay_out);
        State {
            sq: new_ring(SQ),
            outbox: Outbox {
                cq: new_ring(CQ),
                spill: new_ring(SPILL),
            },
            snapshot: 0,
            boot: [0; BOOT_WORDS],
        }
    })
}

// ---- Rings ----
//
// Commands and their effects cross in rings (crates/xpute/core/src/ipc/ring.rs):
// a message names a command (pxl_shared::spec::cmd) and an XTP packet in the
// ring's slot, which the kernel reads where it lies. The shell assembles
// what it submits and takes apart what completes; nothing here is shaped
// for it. The shell pushes into the submission ring, rings `interrupt` with
// the turn's quota, and reads the completion ring when the call returns.

/// Words in the boot block: the section the module keeps for itself at the
/// front of the memory; where the submission ring and the completion ring
/// are; then the fixed, page and mesh sections, each its offset, its bytes
/// and its pool's region size; then the quantum the kernel declares to the
/// central scheduler (xpute_core::sched::quantum's QuantumPolicy): margin
/// share, min, max and batch ms, and the settle time, each in thousandths.
pub const BOOT_WORDS: usize = 15;

/// The boot word holding the address of the heap's row (`BuddyMalloc`'s
/// `stat`): the range it spans, the top it has reached and what it has served.
const BOOT_HEAP: usize = 14;

const BOOT_QUANTUM: usize = 9;

/// Lays out the memory the module was given, the first time, and hands out
/// the boot block's address: what the shell reads once, before any ring.
#[no_mangle]
pub extern "C" fn boot() -> *const u32 {
    install_host();
    let map = mem::map().unwrap_or_else(lay_out);
    let s = state();
    let (sq, cq) = (ring_address(&s.sq), ring_address(&s.outbox.cq));
    s.boot[..3].copy_from_slice(&[data_end(), sq, cq]);
    s.boot[BOOT_HEAP] = ALLOC.stat() as u32;
    for (i, sec) in [map.fixed, map.heap].into_iter().enumerate() {
        write_section(&mut s.boot, 3 + i * SECTION_WORDS as usize, sec);
    }
    let q = pxl_core::module::world::sched::tick::QUANTUM;
    for (i, v) in [q.margin_share, q.min_ms, q.max_ms, q.batch_ms, q.settle_ms].into_iter().enumerate() {
        s.boot[BOOT_QUANTUM + i] = (v * 1000.0).round() as u32;
    }
    s.boot.as_ptr()
}

/// Cuts the memory the central scheduler gave into the kernel's map, and
/// makes the volumes over it.
///
/// Nothing is cut here any more. The sections are ranges the map names and
/// their contents' own affair; the heap is the one allocator and it knows its
/// range without being told (`buddy_tree`'s `Range`), so laying out is
/// recording what the map says and opening the volumes over it.
fn lay_out() -> mem::Map {
    let stack_high = core::ptr::addr_of!(__stack_high) as u32;
    assert!(
        stack_high == pxl_core::mem::spec::STACK_BYTES,
        "the stack was linked to end at {stack_high}, not at mem.json's STACK_BYTES"
    );
    let given = core::arch::wasm32::memory_size(0) as u64 * (1 << 16);
    let map = mem::lay_out(given.min(u32::MAX as u64) as u32, data_end());
    // SAFETY: the memory is this module's own, addressed from 0.
    kernel().volumes = Some(unsafe { Volumes::new(core::ptr::null_mut()) });
    mem::set_map(map);
    map
}

/// The doorbell: the rising edge of a turn, with the time it may take. The
/// kernel applies the submissions waiting — a frame among them runs its tick
/// on what is left of `quota_ms` — then polls the tasks queued for the turn
/// (executor.rs), and the return is the falling edge: when
/// the kernel wants the next turn, in ms from now, 0 for the next frame and
/// xpute_core::sched::quantum's NO_WAKE for none. A turn rung with no quota
/// only applies what waits.
#[no_mangle]
pub extern "C" fn interrupt(quota_ms: f64) -> f64 {
    let s = state();
    pxl_core::module::world::change::world_edge().rise(quota_ms);
    drain(s);
    executor::poll();
    pxl_core::module::world::change::world_edge().fall()
}

/// The UI's snapshot: what WORLD_SNAPSHOT names instead of carrying.
static SNAPSHOT: Held<u8> = Held::new();

/// A packet's bytes written in place into a ring slot: its length, or none
/// when it did not fit one.
type Write<'a> = &'a dyn Fn(&mut [u8]) -> Option<u32>;

/// What a signal names rather than carries: a block the kernel holds and
/// grows, valid until the next signal of the same command fills it again —
/// the shell reads it before the next turn can make another, and there is no
/// block to hand back.
struct Held<T>(core::cell::UnsafeCell<Vec<T>>);

// SAFETY: the module runs on one thread, and what is held is read before
// anything fills it again.
unsafe impl<T> Sync for Held<T> {}

impl<T: Copy + Default> Held<T> {
    const fn new() -> Held<T> {
        Held(core::cell::UnsafeCell::new(Vec::new()))
    }

    /// Fills it with `len` of `value`, and answers where it lies and how many.
    ///
    /// # Safety
    /// Nothing may hold a reference into it from an earlier fill.
    unsafe fn fill(&self, len: usize, mut value: impl FnMut(usize) -> T) -> (u32, u32) {
        // SAFETY: see Sync.
        let v = unsafe { &mut *self.0.get() };
        v.clear();
        v.resize(len, T::default());
        for (i, slot) in v.iter_mut().enumerate() {
            *slot = value(i);
        }
        (v.as_ptr() as u32, len as u32)
    }
}

/// Where what the kernel posts goes: the completion ring while it has room
/// and nothing waits before it, otherwise the spill ring behind it, so what
/// was raised first is read first; `flush` moves the spill on once the shell
/// has read. Each message is written once, in place, and nothing is held on
/// a heap. A spill that fills as well is a sizing fault, as a full pool is.
struct Outbox {
    cq: Ring,
    spill: Ring,
}

impl Outbox {
    fn post(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: Option<Write<'_>>) {
        let push = |r: &Ring| match write {
            Some(w) => r.push_in_place(tag, cmd, flags, result, w),
            None => r.push(tag, cmd, flags, None, result),
        };
        if self.spill.is_empty() && push(&self.cq) == Errno::OK {
            pxl_core::module::world::sched::ledger::ledger_note_rings(self.cq.len(), 0);
            return;
        }
        let e = push(&self.spill);
        pxl_core::module::world::sched::ledger::ledger_note_rings(self.cq.len(), self.spill.len());
        assert!(e == Errno::OK, "the completion ring and its spill are full: 0x{cmd:x} has nowhere to go (errno {})", e as i32);
    }

    /// Whether another command's worth of signals has somewhere to go. What
    /// `drain` stops on: a command that has not been applied can be left in
    /// the submission ring for the next call, where what it would have raised
    /// cannot be left anywhere.
    ///
    /// The margin is the spill rather than the completion ring, so the shell
    /// is asked to read before anything is at risk — io_uring answers a
    /// submission `-EBUSY` while its backlog stands, for the same reason.
    fn has_room(&self) -> bool {
        self.spill.len() + MOST_SIGNALS <= self.spill.capacity
    }

    /// Moves what fits of the spill into the completion ring, in order.
    fn flush(&self) {
        loop {
            let at = self.spill.peek();
            if at < 0 {
                return;
            }
            let at = at as u32;
            let pkt = self.spill.packet(at).expect("a spilled packet is the kernel's own");
            if self.cq.push(self.spill.tag(at), self.spill.cmd(at), self.spill.flags(at), pkt.as_ref(), self.spill.result(at)) != Errno::OK {
                return;
            }
            self.spill.advance();
        }
    }
}

/// Signals one command may raise before `drain` checks again. A frame is the
/// many-signalled one — its toasts, sensor asks, snapshot, fetches and loads —
/// so this bounds a single command rather than a turn.
const MOST_SIGNALS: u32 = 64;

/// A signal for the shell with a packet of `count` children written by `f`.
fn raise(o: &Outbox, cmd: Cmd, count: u32, f: impl Fn(&mut PacketWriter)) {
    o.post(
        0,
        cmd as u32,
        0,
        0,
        Some(&|out: &mut [u8]| {
            let mut w = PacketWriter::new(out, count);
            f(&mut w);
            w.finish()
        }),
    );
}

/// What the kernel asks of the shell since it last asked, raised as signals:
/// toasts, the sensors' watches, console lines, the UI's snapshot when it
/// changed, the fetches to start and cancel, and the loads.
fn post_shell(s: &mut State) {
    use pxl_core::module::device::pose::device_orientation::{take_orientation_commands, OrientationCommand};
    use pxl_core::module::device::pose::geolocation::{take_geolocation_commands, GeolocationCommand};
    use pxl_core::module::map::ui::toast::toast_controller::{take_toast_commands, ToastCommand, ToastTone};
    use pxl_core::module::world::gltf_cache::{take_load_requests, LoadKind};
    use pxl_core::module::world::sched::fetch_scheduler::{fetch_take_aborts, fetch_take_started};

    const GEOLOCATION: u32 = 0;
    const ORIENTATION: u32 = 1;
    let sensor = |cmd: Cmd, which: u32| {
        raise(&s.outbox, cmd, 1, |e| {
            e.u32(Some(which));
        })
    };
    for c in take_toast_commands() {
        raise(&s.outbox, Cmd::WORLD_TOAST, 4, |e| match &c {
            ToastCommand::Show(spec) => {
                let tone = match spec.tone {
                    ToastTone::Loading => 0,
                    ToastTone::Error => 1,
                    ToastTone::Success => 2,
                };
                e.bool(Some(true)).str(Some(spec.id)).u32(Some(spec.message as u32)).u32(Some(tone));
            }
            ToastCommand::Dismiss(id) => {
                e.bool(Some(false)).str(Some(id)).str(Some("")).u32(Some(0));
            }
        });
    }
    for c in take_geolocation_commands() {
        match c {
            GeolocationCommand::Start => sensor(Cmd::SENSOR_CREATE, GEOLOCATION),
            GeolocationCommand::Stop => sensor(Cmd::SENSOR_DELETE, GEOLOCATION),
        }
    }
    for c in take_orientation_commands() {
        match c {
            OrientationCommand::Listen => sensor(Cmd::SENSOR_CREATE, ORIENTATION),
            OrientationCommand::RequestPermission => sensor(Cmd::SENSOR_PERMISSION, ORIENTATION),
            OrientationCommand::Remove => sensor(Cmd::SENSOR_DELETE, ORIENTATION),
        }
    }
    let mut text = String::new();
    pxl_core::app::shell::ui_snapshot_json(&mut text).expect("a UI snapshot");
    let hash = xpute_core::kit::hash::fnv1a64_str(&text);
    if hash != s.snapshot {
        s.snapshot = hash;
        // The JSON is far past a slot and the shell reads it before the next
        // turn can make another, so it crosses as an address and its bytes.
        // SAFETY: the last snapshot, read by the shell before this one.
        let (at, len) = unsafe { SNAPSHOT.fill(text.len(), |i| text.as_bytes()[i]) };
        raise(&s.outbox, Cmd::WORLD_SNAPSHOT, 2, |e| {
            e.u32(Some(at)).u32(Some(len));
        });
    }
    // The key crosses as the one word the queue orders by; the shell holds it
    // as a handle and hands it back, and never reads into it.
    fetch_take_started(|d| {
        let key = pxl_core::module::world::sched::fetch_scheduler::hash_of(d.key);
        raise(&s.outbox, Cmd::FETCH_CREATE, 2, |e| {
            e.u64(Some(key)).str_display(pxl_core::module::world::fetch::DemandUrl(d));
        });
    });
    fetch_take_aborts(|name| {
        let key = pxl_core::module::world::sched::fetch_scheduler::hash_of(name);
        raise(&s.outbox, Cmd::FETCH_DELETE, 1, |e| {
            e.u64(Some(key));
        });
    });
    for q in take_load_requests() {
        let kind = match q.kind {
            LoadKind::Gltf => 0,
            LoadKind::Image => 1,
        };
        raise(&s.outbox, Cmd::LOAD_CREATE, 2, |e| {
            e.u32(Some(kind)).str(Some(q.url.as_str()));
        });
    }
}

/// The end of a frame, the kernel having drawn it: what it asks of the shell,
/// and last the draws it listed.
fn post_turn(s: &mut State) {
    post_shell(s);
    let draws = kernel().sched.draw.draw_count();
    raise(&s.outbox, Cmd::WORLD_UPDATE, 1, |b| {
        b.u32(Some(draws));
    });
}

/// A packet's branch of children, refused when the command needs one and
/// the message has none.
fn fields(pkt: &Option<U8Array>) -> Result<BranchCursor, MarshalError> {
    match pkt {
        Some(pkt) => TreeReader::new(pkt.clone())?.read_branch(),
        None => Err(MarshalError::new(Errno::EBADMSG, Some("command without its packet"), None)),
    }
}

fn u32_at(b: &BranchCursor, i: u32) -> Result<u32, MarshalError> {
    Ok(b.at(i)?.get_number()? as u32)
}

fn i32_at(b: &BranchCursor, i: u32) -> Result<i32, MarshalError> {
    Ok(b.at(i)?.get_number()? as i32)
}

fn number_at(b: &BranchCursor, i: u32) -> Result<f64, MarshalError> {
    b.at(i)?.get_number()
}

fn u64_at(b: &BranchCursor, i: u32) -> Result<u64, MarshalError> {
    Ok(b.at(i)?.get_bigint()? as u64)
}

fn bool_at(b: &BranchCursor, i: u32) -> Result<bool, MarshalError> {
    b.at(i)?.get_bool()
}

/// Applies one submission: its reply's value, or none for a command nobody
/// numbers or a signal submitted as a command, which replies ENOSYS; a packet
/// that does not read as the command's replies with its errno.
fn apply(s: &mut State, number: u32, pkt: &Option<U8Array>) -> Result<Option<i32>, MarshalError> {
    let Some(cmd) = Cmd::of(number) else { return Ok(None) };
    match cmd {
        Cmd::NOP => {
            let value = match pkt {
                Some(_) => u32_at(&fields(pkt)?, 0)?,
                None => 0,
            };
            Ok(Some(value as i32))
        }
        Cmd::WORLD_UPDATE => {
            let f = fields(pkt)?;
            let platform = pxl_core::module::world::world::Platform {
                geolocation: bool_at(&f, 4)?,
                orientation: pxl_core::module::device::pose::device_orientation::OrientationPlatform {
                    supported: bool_at(&f, 5)?,
                    needs_permission: bool_at(&f, 6)?,
                },
            };
            let host = pxl_core::module::world::world::HostFrame {
                width: number_at(&f, 0)?,
                height: number_at(&f, 1)?,
                dpr: number_at(&f, 2)?,
                delta: number_at(&f, 3)?,
                platform,
                frame_ms: number_at(&f, 7)?,
                interactive: bool_at(&f, 8)?,
                credits: u32_at(&f, 9)?,
            };
            let edge = pxl_core::module::world::change::world_edge();
            pxl_core::module::world::world::world_frame(host, edge.quota_ms(), edge.edge_at());
            post_turn(s);
            Ok(Some(0))
        }
        Cmd::WORLD_CREATE => {
            let f = fields(pkt)?;
            pxl_core::lib::env::config::set_build_info(pxl_core::lib::env::config::BuildInfo {
                app_env: f.at(0)?.get_str()?,
                build_time: f.at(1)?.get_str()?,
                build_id: f.at(2)?.get_str()?,
                build_number: f.at(3)?.get_str()?,
                published_tiles: bool_at(&f, 4)?,
            });
            pxl_core::sys::host::set_handheld(bool_at(&f, 5)?);
            pxl_core::module::world::object::store::load();
            Ok(Some(0))
        }
        Cmd::WORLD_ACTION => {
            let f = fields(pkt)?;
            let text = f.at(0)?;
            pxl_core::app::shell::ui_action(text.get_str_ref()?);
            post_shell(s);
            Ok(Some(0))
        }
        Cmd::INPUT_UPDATE => {
            let f = fields(pkt)?;
            pxl_core::module::camera::input::camera_input_rect(pxl_core::module::camera::input::Rect {
                left: number_at(&f, 0)?,
                top: number_at(&f, 1)?,
                width: number_at(&f, 2)?,
                height: number_at(&f, 3)?,
            });
            Ok(Some(0))
        }
        Cmd::INPUT_POINTER => {
            use pxl_core::module::camera::input::{camera_pointer_down, camera_pointer_move, camera_pointer_out, camera_pointer_up, PointerKind};
            let f = fields(pkt)?;
            let kind = match u32_at(&f, 1)? {
                1 => PointerKind::Touch,
                2 => PointerKind::Pen,
                _ => PointerKind::Mouse,
            };
            let (id, button, x, y) = (i32_at(&f, 2)?, i32_at(&f, 3)?, number_at(&f, 4)?, number_at(&f, 5)?);
            match u32_at(&f, 0)? {
                0 => camera_pointer_down(kind, id, button, x, y),
                1 => camera_pointer_move(kind, id, x, y),
                2 => camera_pointer_up(kind, id),
                _ => camera_pointer_out(),
            }
            Ok(Some(0))
        }
        Cmd::INPUT_WHEEL => {
            pxl_core::module::camera::input::camera_wheel(number_at(&fields(pkt)?, 0)?);
            Ok(Some(0))
        }
        Cmd::SENSOR_UPDATE => {
            let f = fields(pkt)?;
            let opt = |v: f64| (!v.is_nan()).then_some(v);
            pxl_core::module::device::pose::geolocation::geolocation_fix(number_at(&f, 0)?, number_at(&f, 1)?, opt(number_at(&f, 2)?), number_at(&f, 3)?, opt(number_at(&f, 4)?));
            Ok(Some(0))
        }
        Cmd::SENSOR_ERROR => {
            use pxl_core::module::device::pose::geolocation::{geolocation_error, GeolocationErrorCode};
            let code = match u32_at(&fields(pkt)?, 0)? {
                1 => GeolocationErrorCode::PermissionDenied,
                2 => GeolocationErrorCode::PositionUnavailable,
                3 => GeolocationErrorCode::Timeout,
                _ => GeolocationErrorCode::Other,
            };
            geolocation_error(code);
            Ok(Some(0))
        }
        Cmd::SENSOR_ORIENTATION => {
            let f = fields(pkt)?;
            let opt = |v: f64| (!v.is_nan()).then_some(v);
            pxl_core::module::device::pose::device_orientation::orientation_event(opt(number_at(&f, 0)?), opt(number_at(&f, 1)?), opt(number_at(&f, 2)?), opt(number_at(&f, 3)?));
            Ok(Some(0))
        }
        Cmd::SENSOR_PERMISSION => {
            let f = fields(pkt)?;
            let (granted, failed) = (bool_at(&f, 1)?, bool_at(&f, 2)?);
            let platform = pxl_core::module::world::world::world_platform();
            match (u32_at(&f, 0)?, granted) {
                (0, true) => pxl_core::module::device::pose::geolocation::geolocation_permission_granted(platform.geolocation),
                (0, false) => {}
                _ => pxl_core::module::device::pose::device_orientation::orientation_permission(granted, failed),
            }
            Ok(Some(0))
        }
        Cmd::SENSOR_VISIBILITY => {
            let visible = bool_at(&fields(pkt)?, 0)?;
            let platform = pxl_core::module::world::world::world_platform();
            pxl_core::module::device::pose::geolocation::geolocation_visibility(visible, platform.geolocation);
            if visible {
                pxl_core::module::device::pose::device_orientation::orientation_visible();
            } else {
                pxl_core::module::world::object::store::flush_save();
            }
            Ok(Some(0))
        }
        Cmd::FETCH_UPDATE => {
            let f = fields(pkt)?;
            pxl_core::module::world::fetch::fetch_arrived(u64_at(&f, 0)?, u32_at(&f, 1)?, u32_at(&f, 2)?, (u32_at(&f, 3)?, u32_at(&f, 4)?), u32_at(&f, 5)?, number_at(&f, 6)?);
            Ok(Some(0))
        }
        Cmd::LOAD_UPDATE => {
            let f = fields(pkt)?;
            let url = f.at(0)?;
            // A model's bytes are far past a slot: the host holds them, and
            // they are read here, in the kernel's turn.
            let mut body = vec![0u8; u32_at(&f, 2)? as usize];
            pxl_core::sys::host::body_read(u32_at(&f, 1)?, &mut body);
            pxl_core::module::world::gltf_cache::gltf_arrived(url.get_str_ref()?, &body);
            Ok(Some(0))
        }
        Cmd::LOAD_IMAGE => {
            let f = fields(pkt)?;
            let url = f.at(0)?;
            pxl_core::module::world::gltf_cache::image_arrived(url.get_str_ref()?, u32_at(&f, 1)?);
            Ok(Some(0))
        }
        Cmd::LOAD_DELETE => {
            let f = fields(pkt)?;
            let url = f.at(0)?;
            pxl_core::module::world::gltf_cache::load_rejected(url.get_str_ref()?);
            Ok(Some(0))
        }
        Cmd::GPU_CREATE => {
            let f = fields(pkt)?;
            pxl_core::render::gpu::renderer::gpu_created(u32_at(&f, 0)?, bool_at(&f, 1)?);
            Ok(Some(0))
        }
        Cmd::GPU_DELETE => {
            pxl_core::render::gpu::renderer::gpu_lost();
            Ok(Some(0))
        }
        Cmd::SURFACE_CREATE => {
            let handle = pxl_core::render::gpu::surface::surface_create(u32_at(&fields(pkt)?, 0)?);
            Ok(Some(handle as i32))
        }
        Cmd::SURFACE_UPDATE => {
            let f = fields(pkt)?;
            pxl_core::render::gpu::surface::surface_update(u32_at(&f, 0)?, u32_at(&f, 1)?, u32_at(&f, 2)?, number_at(&f, 3)?);
            Ok(Some(0))
        }
        Cmd::SURFACE_DELETE => {
            if let Some(closed) = pxl_core::render::gpu::surface::surface_delete(u32_at(&fields(pkt)?, 0)?) {
                pxl_core::render::gpu::renderer::surface_closed(closed);
            }
            Ok(Some(0))
        }
        Cmd::WORLD_TOAST | Cmd::WORLD_SNAPSHOT | Cmd::SENSOR_CREATE | Cmd::SENSOR_DELETE | Cmd::FETCH_CREATE | Cmd::FETCH_DELETE | Cmd::LOAD_CREATE => Ok(None),
    }
}

/// Applies every submission waiting. What they make goes into the completion
/// ring as far as it has room; the rest waits in the module, in order, for
/// the next call — nothing is dropped, and nothing sent waits to be applied,
/// which is what lets a read see every write before it. Replies to what asks
/// (`ACKREQ`), and to what failed — a command nobody numbers, a packet that
/// does not read — since a command's effect is otherwise its signals.
fn drain(s: &mut State) -> u32 {
    let tail = s.outbox.cq.tail();
    s.outbox.flush();
    loop {
        if !s.outbox.has_room() {
            // Backpressure rather than a panic: what is left stays in the
            // submission ring, in order, and the shell reads the completions
            // before it is drained again.
            break;
        }
        let at = s.sq.peek();
        if at < 0 {
            break;
        }
        let at = at as u32;
        let (tag, number, flags) = (s.sq.tag(at), s.sq.cmd(at), s.sq.flags(at));
        let result = match s.sq.packet(at) {
            Ok(pkt) => match apply(s, number, &pkt) {
                Ok(Some(value)) => Ok(value),
                Ok(None) => Err(Errno::ENOSYS as i32),
                Err(e) => Err(e.errno as i32),
            },
            Err(e) => Err(e.errno as i32),
        };
        s.sq.advance();
        if flags & FrameFlag::ACKREQ as u32 != 0 || result.is_err() {
            s.outbox.post(tag, number, FrameFlag::RES as u32, result.unwrap_or_else(|errno| errno), None);
        }
    }
    s.outbox.cq.tail().wrapping_sub(tail)
}

// ---- The host's hooks (pxl_core::sys::host, pxl_core::sys::global) ----

extern "C" {
    /// `localStorage.getItem(key)`'s length in UTF-8 bytes, or -1 for none.
    #[link_name = "storage_len"]
    fn host_storage_len(key: *const u8, key_len: usize) -> i32;
    /// Copies the item's UTF-8 bytes into `dst`, at most `cap` of them.
    #[link_name = "storage_get"]
    fn host_storage_get(key: *const u8, key_len: usize, dst: *mut u8, cap: usize);
    /// `localStorage.setItem(key, value)`.
    #[link_name = "storage_set"]
    fn host_storage_set(key: *const u8, key_len: usize, value: *const u8, value_len: usize);
    /// Writes the local wall clock's five doubles at `out`: unix ms, hours,
    /// minutes, seconds, and the local day's start in unix ms.
    #[link_name = "local_time"]
    fn host_local_time(out: *mut f64);
    /// A canvas context's `measureText(text).width` in `font`.
    #[link_name = "measure_text"]
    fn host_measure_text(font: *const u8, font_len: usize, text: *const u8, text_len: usize) -> f64;
    /// 32 random bits.
    #[link_name = "entropy"]
    fn host_entropy() -> u32;
    /// Copies the body the host holds as `body` into `dst`, `len` bytes, and
    /// lets it go; a `len` of 0 only lets it go.
    #[link_name = "body_read"]
    fn host_body_read(body: u32, dst: *mut u8, len: usize);
}

// ---- The host's WebGPU (pxl_core::render::gpu::api, apps/web/src/wasm/gpu.ts) ----

extern "C" {
    #[link_name = "gpu_reset"]
    fn host_gpu_reset();
    #[link_name = "gpu_buffer"]
    fn host_gpu_buffer(id: u32, size: u32, usage: u32);
    #[link_name = "gpu_texture"]
    fn host_gpu_texture(id: u32, width: u32, height: u32, layers: u32, format: u32, mips: u32, samples: u32, usage: u32);
    #[link_name = "gpu_image_texture"]
    fn host_gpu_image_texture(id: u32, image: u32) -> u32;
    #[link_name = "gpu_view"]
    fn host_gpu_view(id: u32, texture: u32, dimension: u32, base_mip: u32, mips: u32, base_layer: u32, layers: u32);
    #[link_name = "gpu_sampler"]
    fn host_gpu_sampler(id: u32, mag: u32, min: u32, mipmap: u32, address: u32, compare: u32, anisotropy: u32);
    #[link_name = "gpu_shader"]
    fn host_gpu_shader(id: u32, code: *const u8, len: usize);
    #[link_name = "gpu_layout"]
    fn host_gpu_layout(id: u32, entries: *const u32, words: usize, samples: u32);
    #[link_name = "gpu_pipeline"]
    fn host_gpu_pipeline(id: u32, record: *const u32, words: usize, layouts: *const u32, count: usize, module: u32, format: u32, samples: u32);
    #[link_name = "gpu_bind_group"]
    fn host_gpu_bind_group(id: u32, layout: u32, entries: *const u32, words: usize);
    #[link_name = "gpu_destroy"]
    fn host_gpu_destroy(id: u32);
    #[link_name = "gpu_write_buffer"]
    fn host_gpu_write_buffer(id: u32, offset: u32, bytes: *const u8, len: usize);
    #[link_name = "gpu_write_texture"]
    fn host_gpu_write_texture(id: u32, layer: u32, width: u32, height: u32, bytes: *const u8, len: usize);
    #[link_name = "gpu_encoder"]
    fn host_gpu_encoder(id: u32);
    #[link_name = "gpu_pass"]
    fn host_gpu_pass(id: u32, encoder: u32, desc: *const u32, words: usize);
    #[link_name = "gpu_set_pipeline"]
    fn host_gpu_set_pipeline(pass: u32, pipeline: u32);
    #[link_name = "gpu_set_bind_group"]
    fn host_gpu_set_bind_group(pass: u32, index: u32, group: u32, offsets: *const u32, count: usize);
    #[link_name = "gpu_set_vertex_buffer"]
    fn host_gpu_set_vertex_buffer(pass: u32, slot: u32, buffer: u32, offset: u32, size: u32);
    #[link_name = "gpu_set_index_buffer"]
    fn host_gpu_set_index_buffer(pass: u32, buffer: u32, offset: u32, size: u32);
    #[link_name = "gpu_draw"]
    fn host_gpu_draw(pass: u32, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32);
    #[link_name = "gpu_draw_indexed"]
    fn host_gpu_draw_indexed(pass: u32, indices: u32, instances: u32, first_index: u32, first_instance: u32);
    #[link_name = "gpu_end"]
    fn host_gpu_end(pass: u32);
    #[link_name = "gpu_submit"]
    fn host_gpu_submit(encoder: u32);
    #[link_name = "gpu_configure"]
    fn host_gpu_configure(surface: u32);
    #[link_name = "gpu_resize"]
    fn host_gpu_resize(surface: u32, width: u32, height: u32);
    #[link_name = "gpu_current"]
    fn host_gpu_current(surface: u32, view: u32);
    #[link_name = "gpu_query_set"]
    fn host_gpu_query_set(id: u32, count: u32);
    #[link_name = "gpu_timestamps"]
    fn host_gpu_timestamps(encoder: u32, query_set: u32);
    #[link_name = "gpu_ms"]
    fn host_gpu_ms() -> f64;
    #[link_name = "gpu_paint"]
    fn host_gpu_paint(ops: *const u32, words: usize);
    #[link_name = "gpu_paint_copy"]
    fn host_gpu_paint_copy(texture: u32, x: u32, y: u32, layer: u32, width: u32, height: u32);
}

/// Hands the renderer the host's WebGPU. Every call passes numbers and the
/// address and length of what it names, which the host reads during the call.
fn install_gpu() {
    use pxl_core::render::gpu::api::{set_gpu_api, GpuApi};
    // SAFETY, for every call below: an import that reads its numbers and the
    // lanes it is given where they lie, and keeps no address past the call.
    set_gpu_api(GpuApi {
        reset: || unsafe { host_gpu_reset() },
        buffer: |id, size, usage| unsafe { host_gpu_buffer(id, size, usage) },
        texture: |id, width, height, layers, format, mips, samples, usage| unsafe { host_gpu_texture(id, width, height, layers, format, mips, samples, usage) },
        image_texture: |id, image| unsafe { host_gpu_image_texture(id, image) != 0 },
        view: |id, texture, dimension, base_mip, mips, base_layer, layers| unsafe { host_gpu_view(id, texture, dimension, base_mip, mips, base_layer, layers) },
        sampler: |id, mag, min, mipmap, address, compare, anisotropy| unsafe { host_gpu_sampler(id, mag, min, mipmap, address, compare, anisotropy) },
        shader: |id, code| unsafe { host_gpu_shader(id, code.as_ptr(), code.len()) },
        layout: |id, entries, samples| unsafe { host_gpu_layout(id, entries.as_ptr(), entries.len(), samples) },
        pipeline: |id, record, layouts, module, format, samples| unsafe { host_gpu_pipeline(id, record.as_ptr(), record.len(), layouts.as_ptr(), layouts.len(), module, format, samples) },
        bind_group: |id, layout, entries| unsafe { host_gpu_bind_group(id, layout, entries.as_ptr(), entries.len()) },
        destroy: |id| unsafe { host_gpu_destroy(id) },
        write_buffer: |id, offset, bytes| unsafe { host_gpu_write_buffer(id, offset, bytes.as_ptr(), bytes.len()) },
        write_texture: |id, layer, width, height, bytes| unsafe { host_gpu_write_texture(id, layer, width, height, bytes.as_ptr(), bytes.len()) },
        encoder: |id| unsafe { host_gpu_encoder(id) },
        pass: |id, encoder, desc| unsafe { host_gpu_pass(id, encoder, desc.as_ptr(), desc.len()) },
        set_pipeline: |pass, pipeline| unsafe { host_gpu_set_pipeline(pass, pipeline) },
        set_bind_group: |pass, index, group, offsets| unsafe { host_gpu_set_bind_group(pass, index, group, offsets.as_ptr(), offsets.len()) },
        set_vertex_buffer: |pass, slot, buffer, offset, size| unsafe { host_gpu_set_vertex_buffer(pass, slot, buffer, offset, size) },
        set_index_buffer: |pass, buffer, offset, size| unsafe { host_gpu_set_index_buffer(pass, buffer, offset, size) },
        draw: |pass, vertices, instances, first_vertex, first_instance| unsafe { host_gpu_draw(pass, vertices, instances, first_vertex, first_instance) },
        draw_indexed: |pass, indices, instances, first_index, first_instance| unsafe { host_gpu_draw_indexed(pass, indices, instances, first_index, first_instance) },
        end: |pass| unsafe { host_gpu_end(pass) },
        submit: |encoder| unsafe { host_gpu_submit(encoder) },
        configure: |surface| unsafe { host_gpu_configure(surface) },
        resize: |surface, width, height| unsafe { host_gpu_resize(surface, width, height) },
        current: |surface, view| unsafe { host_gpu_current(surface, view) },
        query_set: |id, count| unsafe { host_gpu_query_set(id, count) },
        timestamps: |encoder, query_set| unsafe { host_gpu_timestamps(encoder, query_set) },
        gpu_ms: || unsafe { host_gpu_ms() },
        paint: |ops| unsafe { host_gpu_paint(ops.as_ptr(), ops.len()) },
        paint_copy: |texture, x, y, layer, width, height| unsafe { host_gpu_paint_copy(texture, x, y, layer, width, height) },
    });
}

fn hook_now() -> f64 {
    // SAFETY: an import with no arguments.
    unsafe { host_now() }
}

fn hook_storage_get(key: &str) -> Option<String> {
    // SAFETY: the host reads `key` where it lies and writes at most the length
    // it just reported into a buffer of that length.
    unsafe {
        let len = host_storage_len(key.as_ptr(), key.len());
        if len < 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        host_storage_get(key.as_ptr(), key.len(), buf.as_mut_ptr(), buf.len());
        String::from_utf8(buf).ok()
    }
}

fn hook_storage_set(key: &str, value: &str) {
    // SAFETY: the host reads both strings where they lie.
    unsafe { host_storage_set(key.as_ptr(), key.len(), value.as_ptr(), value.len()) }
}

fn hook_local_time() -> pxl_core::sys::host::LocalTime {
    let mut t = [0.0f64; 5];
    // SAFETY: the host writes five doubles into `t`.
    unsafe { host_local_time(t.as_mut_ptr()) };
    pxl_core::sys::host::LocalTime {
        unix_ms: t[0],
        hours: t[1],
        minutes: t[2],
        seconds: t[3],
        day_start_ms: t[4],
    }
}

fn hook_measure_text(font: &str, text: &str) -> f64 {
    // SAFETY: the host reads both strings where they lie.
    unsafe { host_measure_text(font.as_ptr(), font.len(), text.as_ptr(), text.len()) }
}

fn hook_body_read(body: u32, dst: &mut [u8]) {
    // SAFETY: the host writes `dst.len()` bytes at most into `dst`.
    unsafe { host_body_read(body, dst.as_mut_ptr(), dst.len()) }
}

fn hook_entropy() -> u64 {
    // SAFETY: an import with no arguments.
    unsafe { ((host_entropy() as u64) << 32) | host_entropy() as u64 }
}

/// Hands the kernel the host's clock, storage, local time, text measure and
/// entropy, before anything reads them.
fn install_host() {
    pxl_core::sys::global::set_clock(hook_now);
    pxl_core::sys::host::set_storage(pxl_core::sys::host::Storage {
        get: hook_storage_get,
        set: hook_storage_set,
    });
    pxl_core::sys::host::set_local_time(hook_local_time);
    pxl_core::sys::host::set_measure_text(hook_measure_text);
    pxl_core::sys::host::set_body_read(hook_body_read);
    xpute_core::kit::rng::set_entropy(hook_entropy);
    pxl_core::sys::host::set_heap_used(|| ALLOC.used());
    pxl_core::sys::host::set_heap_releases(|| ALLOC.releases());
    pxl_core::sys::host::set_heap_live(|| ALLOC.live());
    pxl_core::sys::host::set_heap_bytes(|| ALLOC.range_bytes());
    pxl_core::sys::host::set_heap_refusals(|| ALLOC.refusals());
    pxl_core::sys::host::set_heap_largest_free(|| ALLOC.largest_free());
    install_gpu();
}

/// The ledger, where it lies: the address of READ's words (ledger.json) —
/// the summary block over the last `window_ms`, an infinite window being the
/// whole recording, and the frame, task and worst-frame rings, each an address
/// and a record count.
///
/// Read in place rather than sent: this is instrumentation, and
/// instrumentation does not go out on the control ring, whose failure policy
/// is that `Outbox::post` asserts when the completion queue and its spill are
/// both full. Nothing is built for the read either — the rings are the ledger's
/// own lanes and the summary a block written where it stays — so a reader
/// allocates nothing from the heap the ledger measures. The addresses are good
/// until the next turn.
#[no_mangle]
pub extern "C" fn ledger_read(window_ms: f64) -> *const u32 {
    use pxl_core::module::world::sched::ledger::{ledger_frames, ledger_tasks, ledger_worst};
    use pxl_core::module::world::sched::ledger_spec::READ;
    static WORDS: pxl_core::sys::global::Global<[u32; READ::WORDS as usize]> = pxl_core::sys::global::Global::new();
    let summary = pxl_core::module::world::sched::summary::ledger_summarize(window_ms);
    let (frames, tasks, worst) = (ledger_frames(), ledger_tasks(), ledger_worst());
    let w = WORDS.get(|| [0; READ::WORDS as usize]);
    w[READ::summary_at as usize] = summary.as_ptr() as u32;
    w[READ::summary_words as usize] = summary.len() as u32;
    w[READ::frames_at as usize] = frames.as_ptr() as u32;
    w[READ::frames_count as usize] = frames.len() as u32;
    w[READ::tasks_at as usize] = tasks.as_ptr() as u32;
    w[READ::tasks_count as usize] = tasks.len() as u32;
    w[READ::worst_at as usize] = worst.as_ptr() as u32;
    w[READ::worst_count as usize] = worst.len() as u32;
    w.as_ptr()
}

/// What the live stream kept since the shell last cleared it: the address of
/// LOG's words (ledger.json), the task rows and the frames, each an address
/// and a count. Zero counts when nothing is waiting, which a shell polling is
/// asking about most of the time.
#[no_mangle]
pub extern "C" fn ledger_log() -> *const u32 {
    use pxl_core::module::world::sched::ledger_spec::LOG;
    static WORDS: pxl_core::sys::global::Global<[u32; LOG::WORDS as usize]> = pxl_core::sys::global::Global::new();
    let (tasks, frames) = pxl_core::module::world::sched::log::ledger_log_rows();
    let w = WORDS.get(|| [0; LOG::WORDS as usize]);
    w[LOG::tasks_at as usize] = tasks.as_ptr() as u32;
    w[LOG::tasks_count as usize] = tasks.len() as u32;
    w[LOG::frames_at as usize] = frames.as_ptr() as u32;
    w[LOG::frames_count as usize] = frames.len() as u32;
    w.as_ptr()
}

/// Forgets what the stream kept, once the shell has read it.
#[no_mangle]
pub extern "C" fn ledger_log_clear() {
    pxl_core::module::world::sched::log::ledger_log_clear();
}
