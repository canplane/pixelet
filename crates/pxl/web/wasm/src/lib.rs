// pxl-wasm/lib.rs

//! The one WebAssembly module: pxl-kernel behind a boot block and a doorbell.
//!
//! No wasm-bindgen. The central scheduler gives the module its memory, and
//! the module lays out everything inside it on its first turn
//! (pxl_kernel::mem); the shell reads the boot block once, at an address both
//! sides know, and everything after crosses in that memory: a command and its
//! packet in the submission ring, its reply and the kernel's signals in the
//! completion ring, what the kernel asks of the host in two streams the host
//! runs after each turn, and `interrupt` to hand over the turn. The clock is
//! the one call the module makes: a turn spends its quota against it while
//! the host cannot run.
//!
//! The module is single-threaded, so the state is a plain global.

mod executor;

use pxl_kernel::abi::cmd::{self as abi, Command, FetchCreateSignal, FetchDeleteSignal, LoadCreateSignal, Sensor, SensorCreateSignal, WorldSnapshotSignal, WorldUpdateSignal};
use pxl_kernel::global::Global;
use pxl_kernel::kernel::kernel;
use pxl_kernel::mem::fixed::Range;
use pxl_kernel::mem::section;
use pxl_render::mesh::volume::Volumes;
use xpute_core::math::scalar::round;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::PacketWriter;
use xpute_runtime::ipc::frame::FrameFlag;
use xpute_runtime::ipc::ring::{ring_bytes, slots_bytes, Ring};
use xpute_runtime::mem::section::{write_section, SECTION_WORDS};

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
    /// two hundred megabytes apart.
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
const HEAP_MIN_LOG2: usize = memory_shift(section::PAGE_BYTES);

const fn memory_shift(bytes: u32) -> usize {
    assert!(bytes.is_power_of_two());
    bytes.trailing_zeros() as usize
}

/// A bit for every node of the tree that can be a parent. The exponents give
/// the length and the compiler will not derive it for us, so it is written
/// once here and `BuddyMalloc` checks it against them on the first request.
const HEAP_SPLIT_BYTES: usize = (1 << (HEAP_MAX_LOG2 - HEAP_MIN_LOG2)) / 8;

impl xpute_core::alloc::buddy_tree::Range for ModuleHeap {
    const MIN_LOG2: usize = HEAP_MIN_LOG2;
    const MAX_LOG2: usize = HEAP_MAX_LOG2;
    type Split = [u8; HEAP_SPLIT_BYTES];
    const SPLIT_INIT: Self::Split = [0; HEAP_SPLIT_BYTES];

    fn base() -> usize {
        // A pure function of the linker's answer and the map's constants, with
        // no state and no order to be in: the first block is asked for long
        // before anything has laid the memory out.
        section::heap_at(data_end()) as usize
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
static ALLOC: xpute_core::alloc::slab::SlabMalloc<ModuleHeap> = xpute_core::alloc::slab::SlabMalloc::new();

struct State {
    /// The submission ring (shell → core), at the address build.rs fixed;
    /// the boot block carries it out to the shell.
    sq: Ring,
    /// The completion ring and its spill.
    outbox: Outbox,
    /// The UI snapshot last posted, by hash: one is posted only when it changed.
    snapshot: u64,
    /// The console stream's rows were handed over, to be let go on the next
    /// turn (LEDGER_TAKE).
    ledger_taken: bool,
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
const _: () = assert!(RING_SLOT_BYTES == section::PAGE_BYTES, "a slot's payload is the page the rest of the memory is counted in");

/// The whole memory as one buffer, which is what a ring's two areas are cut
/// from: the descriptors near the bottom and the slots' payloads past them, at
/// addresses build.rs fixed rather than anyone hands out.
///
/// # Safety
/// The module's own memory, addressed from 0.
unsafe fn module_memory() -> *mut u8 {
    // An address, not a pointer into anything: the linear memory is the
    // machine's and address 0 is its base. `null_mut()` would say the same
    // number and mean something else — offsetting a null pointer is undefined,
    // and the optimizer is free to drop the stores that follow, which it does:
    // a ring's capacity word went missing from an otherwise correct header.
    core::ptr::with_exposed_provenance_mut(0)
}

/// The nth ring, laid out empty.
fn new_ring(n: u32) -> Ring {
    // SAFETY: the module's own memory; the two ranges are build.rs's and no
    // one else's, the linker included.
    let mem = unsafe { module_memory() };
    // SAFETY: the module's memory never moves and outlives every ring over it.
    unsafe { Ring::init(mem, RING_DESC_AT + n * RING_DESC_STRIDE, RING_CAPACITY, RING_SLOT_AT + n * RING_SLOT_STRIDE, RING_SLOT_BYTES) }
}

/// Where a ring's words start in the memory.
fn ring_address(ring: &Ring) -> u32 {
    ring.words().as_ptr() as u32
}

/// The module's one state, made on first use. xpute's device for it rather
/// than a `static mut`, whose references the compiler now warns of: the module
/// is single-threaded and no export re-enters another, which is the promise
/// `Global` is built on.
static STATE: Global<State> = Global::new();

// `wasm_import_module` is what makes this an import, and it is load-bearing
// rather than documentation. wasm-ld used to be handed `--allow-undefined`,
// under which any symbol nothing defined became an import of the module named
// `env` — which is the module the shell instantiates with
// (apps/web/src/kernel/module.ts). Rust stopped passing that flag in 1.96, so
// an extern block that says nothing is a link error.
#[link(wasm_import_module = "env")]
extern "C" {
    /// The client's clock, in milliseconds: what a tick's budget is spent against.
    #[link_name = "now"]
    fn host_now() -> f64;
}

fn state() -> &'static mut State {
    STATE.get(|| State {
        sq: new_ring(SQ),
        outbox: Outbox {
            cq: new_ring(CQ),
            spill: new_ring(SPILL),
        },
        snapshot: 0,
        ledger_taken: false,
    })
}

// ---- Rings ----
//
// Commands and their effects cross in rings (crates/xpute/runtime/src/ipc/ring.rs):
// a message names a command (pxl_kernel::abi::cmd) and an XTP packet in the
// ring's slot, which the kernel reads where it lies. The shell assembles
// what it submits and takes apart what completes; nothing here is shaped
// for it. The shell pushes into the submission ring, rings `interrupt` with
// the turn's quota, and reads the completion ring when the call returns.

/// Words in the boot block: where the module's own data ends; where the
/// submission ring and the completion ring are; then `.fixed` and the heap,
/// each its offset, its bytes and the page it begins on; then the quantum the kernel declares to the
/// central scheduler (xpute_runtime::sched::quantum's QuantumPolicy): margin
/// share, batch frames and the settle time, each in thousandths; then the
/// heap's row; then the two streams the shell runs after every turn, the
/// renderer's (pxl_render::gpu::api) and the host's (pxl_kernel::host).
pub const BOOT_WORDS: usize = 15;

/// The boot words holding the addresses of the renderer's and the host's
/// streams.
const BOOT_GPU: usize = 13;
const BOOT_HOST: usize = 14;

/// The boot word holding the address of the heap's row (`BuddyMalloc`'s
/// `stat`): the range it spans, the top it has reached and what it has served.
const BOOT_HEAP: usize = 12;

const BOOT_QUANTUM: usize = 9;

/// The first turn's: lays out the memory the module was given and writes the
/// boot block where mem.json's BOOT_AT says, for the shell to read once
/// before any ring.
fn boot() {
    install_host();
    executor::install();
    // A broken invariant the kernel names reports itself (`bug!`); one core
    // raises — an index past a slice, an `unwrap` of nothing — reports its
    // place alone, the errno being that no invariant said why.
    xpute_core::status::bug::set_report(pxl_kernel::host::host_bug);
    std::panic::set_hook(Box::new(|info| {
        if let Some(at) = info.location() {
            pxl_kernel::host::host_bug(Errno::ENOTRECOVERABLE, at, 0, 0);
        }
    }));
    let map = lay_out();
    pxl_kernel::sched::turn::boot();
    let s = state();
    // SAFETY: the page at BOOT_AT, which build.rs keeps clear of the rings
    // and the module's data; the boot block's alone.
    let block = unsafe { &mut *(module_memory().add(pxl_kernel::mem::spec::BOOT_AT as usize) as *mut [u32; BOOT_WORDS]) };
    block[..3].copy_from_slice(&[data_end(), ring_address(&s.sq), ring_address(&s.outbox.cq)]);
    block[BOOT_HEAP] = ALLOC.stat() as u32;
    block[BOOT_GPU] = fixed_at(Range::Gpu);
    block[BOOT_HOST] = fixed_at(Range::Host);
    for (i, sec) in [map.fixed, map.heap].into_iter().enumerate() {
        write_section(block, 3 + i * SECTION_WORDS as usize, sec);
    }
    let q = pxl_kernel::sched::tick::QUANTUM;
    for (i, v) in [q.margin_share, q.batch_frames, q.settle_ms].into_iter().enumerate() {
        block[BOOT_QUANTUM + i] = round(v * 1000.0) as u32;
    }
}

/// Where fixed range `r` begins, the map laid.
fn fixed_at(r: Range) -> u32 {
    pxl_kernel::mem::fixed::range(r).or_bug(Errno::ENOTRECOVERABLE).offset
}

/// Records the kernel's map over the memory the central scheduler gave, and
/// makes the volumes over it.
///
/// Nothing is cut here. The sections are ranges the map names and their
/// contents' own affair; the heap is the one allocator and it knows its range
/// without being told (`buddy_tree`'s `Range`).
fn lay_out() -> section::Map {
    let stack_high = core::ptr::addr_of!(__stack_high) as u32;
    xpute_core::ensure!(stack_high == pxl_kernel::mem::spec::STACK_BYTES, ENOTRECOVERABLE, stack_high);
    let given = core::arch::wasm32::memory_size(0) as u64 * (1 << 16);
    let map = section::lay_out(given.min(u32::MAX as u64) as u32, data_end());
    // SAFETY: the memory is this module's own, addressed from 0.
    kernel().volumes = Some(unsafe { Volumes::new(core::ptr::null_mut()) });
    section::set_map(map);
    // SAFETY: `.fixed.gpu`, `.fixed.host` and `.fixed.blocks`, each what it
    // is installed as from a page, zeroed, the renderer's and the host's
    // alone.
    unsafe {
        pxl_render::gpu::api::set_gpu_stream(fixed_at(Range::Gpu));
        pxl_kernel::host::set_host_stream(fixed_at(Range::Host));
        pxl_render::gpu::renderer::set_blocks(fixed_at(Range::Blocks));
    }
    map
}

/// The doorbell: the rising edge of a turn, with the time it may take. The
/// kernel applies the submissions waiting, then polls its tasks on the quota
/// — the frame among them, when the host asked for one — and the return is
/// the falling edge: when the kernel wants the next turn, in ms from now, 0
/// for the next frame and xpute_runtime::sched::quantum's NO_WAKE for none. A
/// turn rung with no quota and no frame only applies what waits. What the
/// kernel asked of the host in the turn is in the two streams by then, for
/// the shell to run before anything else can reach the device or the next
/// turn can begin. The first turn boots.
#[no_mangle]
pub extern "C" fn interrupt(quota_ms: f64) -> f64 {
    use pxl_kernel::sched::turn::{take_world_drawn, world_frame_waiting};
    if section::map().is_none() {
        boot();
    }
    let s = state();
    if core::mem::take(&mut s.ledger_taken) {
        pxl_kernel::sched::ledger::log::ledger_log_clear();
    }
    pxl_kernel::host::host_rise();
    pxl_kernel::world::change::world_edge().rise(quota_ms);
    drain(s);
    if quota_ms > 0.0 || world_frame_waiting() {
        xpute_runtime::sched::task::poll();
    }
    if take_world_drawn() {
        post_turn(s);
    }
    pxl_render::gpu::api::gpu_publish();
    pxl_kernel::host::host_fall();
    pxl_kernel::world::change::world_fall()
}

/// The UI's snapshot, a Snapshot packet written in place turn after turn:
/// what WORLD_SNAPSHOT names instead of carrying. The shell reads it when it
/// hears the signal, before the next turn can write it again, so there is no
/// block to hand back. It grows to the largest snapshot and stays there, so
/// a settled turn asks the heap for nothing.
static SNAPSHOT: Global<Vec<u8>> = Global::new();

/// A packet's bytes written in place into a ring slot: its length, or none
/// when it did not fit one.
type Write<'a> = &'a dyn Fn(&mut [u8]) -> Option<u32>;

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
            pxl_kernel::sched::ledger::record::ledger_note_rings(self.cq.len(), 0);
            return;
        }
        let e = push(&self.spill);
        pxl_kernel::sched::ledger::record::ledger_note_rings(self.cq.len(), self.spill.len());
        xpute_core::ensure!(e == Errno::OK, ENOSPC, cmd, e as i32);
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
            let pkt = self.spill.packet(at).or_bug(Errno::ENOTRECOVERABLE);
            if self.cq.push(self.spill.tag(at), self.spill.cmd(at), self.spill.flags(at), pkt, self.spill.result(at)) != Errno::OK {
                return;
            }
            self.spill.advance();
        }
    }
}

/// Signals one command may raise before `drain` checks again. A frame is the
/// many-signaled one — its sensor asks, snapshot, fetches and loads —
/// so this bounds a single command rather than a turn.
const MOST_SIGNALS: u32 = 64;

/// A signal for the shell with a packet of `count` children written by `f`.
fn raise(o: &Outbox, cmd: Command, count: u32, f: impl Fn(&mut PacketWriter)) {
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
/// the sensors' watches, console lines, the UI's snapshot when it
/// changed, the fetches to start and cancel, and the loads.
fn post_shell(s: &mut State) {
    use pxl_kernel::control::sensor::device_orientation::{take_orientation_commands, OrientationCommand};
    use pxl_kernel::control::sensor::geolocation::{take_geolocation_commands, GeolocationCommand};
    use pxl_kernel::io::sched::{take_started, take_stops};

    let sensor = |cmd: Command, sensor: Sensor| raise(&s.outbox, cmd, SensorCreateSignal::FIELDS, |w| SensorCreateSignal { sensor }.write(w));
    for c in take_geolocation_commands() {
        match c {
            GeolocationCommand::Start => sensor(Command::SENSOR_CREATE, Sensor::Geolocation),
            GeolocationCommand::Stop => sensor(Command::SENSOR_DELETE, Sensor::Geolocation),
        }
    }
    for c in take_orientation_commands() {
        match c {
            OrientationCommand::Listen => sensor(Command::SENSOR_CREATE, Sensor::Orientation),
            OrientationCommand::Remove => sensor(Command::SENSOR_DELETE, Sensor::Orientation),
        }
    }
    let buf = SNAPSHOT.get(Vec::new);
    let len = loop {
        if let Some(len) = pxl_kernel::shell::shell::ui_snapshot(buf) {
            break len;
        }
        let room = (buf.len() * 2).max(section::PAGE_BYTES as usize);
        buf.resize(room, 0);
    };
    let mut h = fnv::FnvHasher::default();
    std::hash::Hasher::write(&mut h, &buf[..len as usize]);
    let hash = std::hash::Hasher::finish(&h);
    if hash != s.snapshot {
        s.snapshot = hash;
        let signal = WorldSnapshotSignal { at: buf.as_ptr() as u32, len };
        raise(&s.outbox, Command::WORLD_SNAPSHOT, WorldSnapshotSignal::FIELDS, |w| signal.write(w));
    }
    // The key crosses as the one word the queue orders by; the shell holds it
    // as a handle and hands it back, and never reads into it.
    take_started(|key, category, subject| match category.bake() {
        Some(bake) => {
            let name = pxl_kernel::io::fetch::DemandName(category, subject);
            raise(&s.outbox, Command::FETCH_CREATE, FetchCreateSignal::FIELDS, |w| FetchCreateSignal { key, bake, name: &name }.write(w));
        }
        // An asset belongs to no bake: it is named by its content, and the
        // host is the one that knows what that is.
        None => {
            let Some((kind, load)) = pxl_kernel::io::load::of_subject(subject) else { return };
            let signal = LoadCreateSignal { key, kind, load: load.number() };
            raise(&s.outbox, Command::LOAD_CREATE, LoadCreateSignal::FIELDS, |w| signal.write(w));
        }
    });
    take_stops(|key| raise(&s.outbox, Command::FETCH_DELETE, FetchDeleteSignal::FIELDS, |w| FetchDeleteSignal { key }.write(w)));
}

/// The end of a frame, the kernel having drawn it: what it asks of the shell,
/// and last the draws it listed.
fn post_turn(s: &mut State) {
    post_shell(s);
    let signal = WorldUpdateSignal {
        draws: kernel().sched.draw.draw_count(),
    };
    raise(&s.outbox, Command::WORLD_UPDATE, WorldUpdateSignal::FIELDS, |w| signal.write(w));
}

/// A command's packet, refused when the command needs one and the message
/// has none.
fn packet<'a>(pkt: &Option<&'a [u8]>) -> Result<&'a [u8], MarshalError> {
    pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))
}

/// A command from the page's UI, applied: a turn asked for, and what it asks
/// of the host raised before the call returns — the shell rings for these at
/// once, so a sensor's close sent after the strobe has stopped is still heard.
fn applied(s: &mut State) -> Result<Option<i32>, MarshalError> {
    pxl_kernel::world::change::request_world_tick();
    post_shell(s);
    Ok(Some(0))
}

/// Applies one submission: its reply's value, or none for a command nobody
/// numbers or a signal submitted as a command, which replies ENOSYS; a packet
/// that does not read as the command's replies with its errno.
fn apply(s: &mut State, number: u32, pkt: &Option<&[u8]>) -> Result<Option<i32>, MarshalError> {
    use pxl_kernel::control::camera::input::{camera_input_rect, camera_pointer_down, camera_pointer_move, camera_pointer_out, camera_pointer_up, camera_wheel, Holding, Rect};
    use pxl_kernel::control::sensor::{device_orientation, geolocation};
    use pxl_kernel::sched::ledger::log::{ledger_set_logging, ledger_window_start, ledger_window_stop};
    use pxl_kernel::sched::turn::world_platform;
    use pxl_kernel::shell::setting::store::{settings_load, settings_store};
    use pxl_kernel::shell::shell as ui;
    use pxl_kernel::world::change::request_world_tick;

    let Some(cmd) = Command::of(number) else { return Ok(None) };
    match cmd {
        Command::NOP => {
            let value = match pkt {
                Some(pkt) => abi::Nop::read(pkt)?.value,
                None => 0,
            };
            Ok(Some(value as i32))
        }
        Command::WORLD_UPDATE => {
            let p = abi::WorldUpdate::read(packet(pkt)?)?;
            pxl_kernel::sched::turn::world_frame_asked(pxl_kernel::sched::turn::HostFrame {
                width: p.width,
                height: p.height,
                dpr: p.dpr,
                delta: p.delta_s,
                frame_ms: p.frame_ms,
                interactive: p.interactive,
                credits: p.credits,
            });
            pxl_kernel::host::set_local_time(pxl_kernel::host::LocalTime {
                unix_ms: p.unix_ms,
                hours: p.hours,
                minutes: p.minutes,
                seconds: p.seconds,
                day_start_ms: p.day_start_ms,
            });
            Ok(Some(0))
        }
        Command::WORLD_CREATE => {
            let p = abi::WorldCreate::read(packet(pkt)?)?;
            pxl_kernel::host::set_development(p.development);
            pxl_kernel::host::set_handheld(p.handheld);
            pxl_kernel::sched::turn::set_world_platform(pxl_kernel::sched::turn::Platform {
                geolocation: p.geolocation,
                orientation: device_orientation::OrientationPlatform {
                    supported: p.orientation,
                    needs_permission: p.orientation_permission,
                },
            });
            settings_load(p.settings.iter().copied());
            if p.objects != 0 {
                pxl_kernel::layer::object::store::load(p.objects, p.objects_bytes);
            }
            Ok(Some(0))
        }
        Command::INPUT_UPDATE => {
            let p = abi::InputUpdate::read(packet(pkt)?)?;
            camera_input_rect(Rect {
                left: p.left,
                top: p.top,
                width: p.width,
                height: p.height,
            });
            Ok(Some(0))
        }
        Command::INPUT_POINTER => {
            use pxl_spec::cell::object::placeable_cube;
            use pxl_spec::cell::DecorId;
            let p = abi::InputPointer::read(packet(pkt)?)?;
            let mut holding = Holding {
                active_tool: p.tool,
                object_size: pxl_kernel::layer::object::size::object_size_of(p.tier),
                ..Holding::default()
            };
            match p.tool {
                abi::ActiveTool::Object => {
                    if let Some(cube) = placeable_cube(p.channel, p.value) {
                        holding.object_cube = cube;
                    }
                }
                abi::ActiveTool::Decor => {
                    if let Some(decor) = u8::try_from(p.value).ok().and_then(DecorId::of) {
                        holding.decor_id = decor;
                    }
                }
                abi::ActiveTool::Pointer | abi::ActiveTool::Eraser => {}
            }
            match p.phase {
                abi::PointerPhase::Down => camera_pointer_down(p.kind, p.id, p.button, p.x, p.y, holding),
                abi::PointerPhase::Move => camera_pointer_move(p.kind, p.id, p.x, p.y, holding),
                abi::PointerPhase::Up => camera_pointer_up(p.kind, p.id, holding),
                abi::PointerPhase::Out => camera_pointer_out(),
            }
            Ok(Some(0))
        }
        Command::INPUT_WHEEL => {
            camera_wheel(abi::InputWheel::read(packet(pkt)?)?.delta_y);
            Ok(Some(0))
        }
        Command::INPUT_TRACKING => {
            ui::tracking_action(abi::InputTracking::read(packet(pkt)?)?.tracking);
            request_world_tick();
            Ok(Some(0))
        }
        Command::INPUT_VIEW => {
            let p = abi::InputView::read(packet(pkt)?)?;
            ui::set_view(p.dist, p.polar);
            applied(s)
        }
        Command::INPUT_RESET_DIST => {
            pxl_kernel::control::camera::store::camera_store().reset_dist();
            applied(s)
        }
        Command::INPUT_COMPASS_RESET => {
            pxl_kernel::shell::ui::compass::compass_reset();
            applied(s)
        }
        Command::INPUT_GO_TO => {
            let p = abi::InputGoTo::read(packet(pkt)?)?;
            ui::go_to_degrees(p.lon, p.lat);
            applied(s)
        }
        Command::SENSOR_UPDATE => {
            let p = abi::SensorUpdate::read(packet(pkt)?)?;
            geolocation::geolocation_fix(p.lon, p.lat, p.altitude, p.accuracy, p.heading);
            Ok(Some(0))
        }
        Command::SENSOR_ERROR => {
            geolocation::geolocation_error(abi::SensorError::read(packet(pkt)?)?.code);
            Ok(Some(0))
        }
        Command::SENSOR_ORIENTATION => {
            let p = abi::SensorOrientation::read(packet(pkt)?)?;
            device_orientation::orientation_event(p.webkit_compass_heading, p.alpha, p.beta, p.gamma);
            Ok(Some(0))
        }
        Command::SENSOR_PERMISSION => {
            let p = abi::SensorPermission::read(packet(pkt)?)?;
            match p.sensor {
                abi::Sensor::Geolocation => {
                    if p.answer == abi::PermissionAnswer::Granted {
                        geolocation::geolocation_permission_granted(world_platform().geolocation);
                    }
                }
                abi::Sensor::Orientation => device_orientation::orientation_permission(p.answer),
            }
            Ok(Some(0))
        }
        Command::SENSOR_VISIBILITY => {
            let visible = abi::SensorVisibility::read(packet(pkt)?)?.visible;
            geolocation::geolocation_visibility(visible, world_platform().geolocation);
            if visible {
                device_orientation::orientation_visible();
            } else {
                pxl_kernel::layer::object::store::flush_save();
            }
            Ok(Some(0))
        }
        Command::SENSOR_ENABLE => {
            ui::sensors_enable();
            applied(s)
        }
        Command::SENSOR_OPEN => {
            ui::sensors_open();
            applied(s)
        }
        Command::SENSOR_CLOSE => {
            ui::sensors_close();
            applied(s)
        }
        Command::FETCH_UPDATE => {
            pxl_kernel::io::fetch::fetch_queued(abi::FetchUpdate::read(packet(pkt)?)?);
            Ok(Some(0))
        }
        Command::LOAD_UPDATE => {
            let p = abi::LoadUpdate::read(packet(pkt)?)?;
            // A model's bytes are far past a slot: the host holds them, and
            // the models' task reads them on a turn's quota.
            pxl_kernel::io::load::gltf_arrived(p.key, p.body, p.bytes);
            Ok(Some(0))
        }
        Command::LOAD_IMAGE => {
            let p = abi::LoadImage::read(packet(pkt)?)?;
            pxl_kernel::io::load::image_arrived(p.key, p.handle);
            Ok(Some(0))
        }
        Command::LOAD_DELETE => {
            pxl_kernel::io::load::load_rejected(abi::LoadDelete::read(packet(pkt)?)?.key);
            Ok(Some(0))
        }
        Command::GPU_CREATE => {
            let p = abi::GpuCreate::read(packet(pkt)?)?;
            pxl_render::gpu::renderer::gpu_created(p.min_uniform_offset_alignment, p.timestamps);
            request_world_tick();
            Ok(Some(0))
        }
        Command::GPU_DELETE => {
            pxl_render::gpu::renderer::gpu_lost();
            Ok(Some(0))
        }
        Command::SURFACE_CREATE => {
            let handle = pxl_render::gpu::surface::surface_create(abi::SurfaceCreate::read(packet(pkt)?)?.role);
            if handle != 0 {
                request_world_tick();
            }
            Ok(Some(handle as i32))
        }
        Command::SURFACE_UPDATE => {
            let p = abi::SurfaceUpdate::read(packet(pkt)?)?;
            if pxl_render::gpu::surface::surface_update(p.surface, p.width, p.height, p.device_pixel_ratio) {
                request_world_tick();
            }
            Ok(Some(0))
        }
        Command::SURFACE_DELETE => {
            if let Some(closed) = pxl_render::gpu::surface::surface_delete(abi::SurfaceDelete::read(packet(pkt)?)?.surface) {
                pxl_render::gpu::renderer::surface_closed(closed);
            }
            Ok(Some(0))
        }
        Command::SETTING_UPDATE => {
            let p = abi::SettingUpdate::read(packet(pkt)?)?;
            settings_store().set_setting(p.key, p.value);
            applied(s)
        }
        Command::SETTING_TOGGLE => {
            settings_store().toggle_setting(abi::SettingToggle::read(packet(pkt)?)?.key);
            applied(s)
        }
        Command::CHAT_CREATE => {
            pxl_kernel::layer::character::chat_bubble::append_chat_line(abi::ChatCreate::read(packet(pkt)?)?.text);
            applied(s)
        }
        Command::PREVIEW_CREATE => {
            let p = abi::PreviewCreate::read(packet(pkt)?)?;
            let number = u8::try_from(p.number).map_err(|_| MarshalError::new(Errno::EINVAL))?;
            if !pxl_kernel::layer::character::preview::preview_open(p.surface, p.group, number) {
                return Err(MarshalError::new(Errno::EINVAL));
            }
            applied(s)
        }
        Command::PREVIEW_DELETE => {
            pxl_kernel::layer::character::preview::preview_close(abi::PreviewDelete::read(packet(pkt)?)?.surface);
            applied(s)
        }
        Command::LEDGER_START => {
            ledger_window_start();
            applied(s)
        }
        Command::LEDGER_STOP => {
            ledger_window_stop();
            applied(s)
        }
        Command::LEDGER_LOG => {
            ledger_set_logging(abi::LedgerLog::read(packet(pkt)?)?.on);
            applied(s)
        }
        // An address in a memory of at most a gibibyte, which an i32 holds.
        Command::LEDGER_READ => Ok(Some(ledger_read(abi::LedgerRead::read(packet(pkt)?)?.window_ms) as i32)),
        Command::LEDGER_TAKE => Ok(Some(ledger_take(s) as i32)),
        Command::WORLD_SNAPSHOT | Command::SENSOR_CREATE | Command::SENSOR_DELETE | Command::FETCH_CREATE | Command::FETCH_DELETE | Command::LOAD_CREATE => Ok(None),
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

// ---- The host's clock (pxl_kernel::host) ----

fn hook_now() -> f64 {
    // SAFETY: an import with no arguments.
    unsafe { host_now() }
}

/// Hands the kernel the host's clock and the allocator's reading, before
/// anything reads them.
fn install_host() {
    pxl_kernel::host::set_clock(hook_now);
    pxl_kernel::host::set_heap_stat(|| pxl_kernel::host::HeapStat {
        range: ALLOC.range_bytes(),
        used: ALLOC.used(),
        live: ALLOC.live(),
        run: ALLOC.run_live(),
        slot: ALLOC.slot_live(),
        largest_free: ALLOC.largest_free(),
        releases: ALLOC.releases(),
        refusals: ALLOC.refusals(),
    });
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
fn ledger_read(window_ms: f64) -> *const u32 {
    use pxl_kernel::sched::ledger::record::{ledger_frames, ledger_tasks, ledger_worst};
    use pxl_kernel::sched::ledger::spec::READ;
    static WORDS: pxl_kernel::global::Global<[u32; READ::WORDS as usize]> = pxl_kernel::global::Global::new();
    let summary = pxl_kernel::sched::ledger::summary::ledger_summarize(window_ms);
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
/// asking about most of the time. The rows are let go on the next turn.
fn ledger_take(s: &mut State) -> *const u32 {
    use pxl_kernel::sched::ledger::spec::LOG;
    static WORDS: pxl_kernel::global::Global<[u32; LOG::WORDS as usize]> = pxl_kernel::global::Global::new();
    let (tasks, frames) = pxl_kernel::sched::ledger::log::ledger_log_rows();
    let w = WORDS.get(|| [0; LOG::WORDS as usize]);
    w[LOG::tasks_at as usize] = tasks.as_ptr() as u32;
    w[LOG::tasks_count as usize] = tasks.len() as u32;
    w[LOG::frames_at as usize] = frames.as_ptr() as u32;
    w[LOG::frames_count as usize] = frames.len() as u32;
    s.ledger_taken = true;
    w.as_ptr()
}
