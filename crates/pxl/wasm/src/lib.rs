//! The one WebAssembly module: pxl-core behind a boot block and a doorbell.
//!
//! No wasm-bindgen. The shell reads the boot block once (`boot`), and
//! everything after crosses in the rings in the core memory, which the
//! module imports and shares with the client (build.rs, fixed_heap.rs): a
//! command and its packet in, its reply and the kernel's signals out, and
//! `interrupt` to hand over the turn. A run set is baked where it lies
//! and a mesh is written into a block of the memory, so nothing is copied
//! across the boundary, and a thread that instantiates the module needs the
//! memory and nothing else.
//!
//! One bake is open at a time, as with the TypeScript context; the module is
//! single-threaded, so the state is a plain global.

use pxl_core::bake::mesher::Step;
use pxl_core::bake::run_set::{run_set_bytes, RunSet};
use pxl_core::bake::scan::{BandView, ExtraView, ScanStep};
use pxl_core::bake::terrain::{PageView, ELE_PADDED_SPAN};
use pxl_core::bake::volume::{Cell, Volumes, Voxel, BAND_WORDS as VOLUME_BAND_WORDS};
use pxl_core::space::constant::{CHUNK_SPAN, PAGE_CELL_SPAN};
use pxl_core::kernel::{kernel, reindex_object, Kernel};
use pxl_core::world::build::{Event as BuildEvent, PageLanes, World};
use pxl_core::world::decor::{Kind as DecorKind, Rule as DecorRule};
use pxl_core::world::object::{Carve, Kind};
use pxl_core::world::pool::Pool;
use pxl_core::world::raycast::Raycast;
use pxl_core::world::sched::{Event, Frame, Scheduler};
use pxl_shared::spec::cmd::{Cmd, StepResult};
use xpute_core::io::marshal::tree::{encoder, BranchCursor, BranchViewImpl, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_core::ipc::cmd::Interrupt;
use xpute_core::ipc::frame::FrameFlag;
use xpute_core::ipc::ring::{ring_bytes, Ring, RING_ENTRY_WORDS, RING_HEADER_WORDS};
use xpute_core::kit::array::ArrayBuffer;
use xpute_core::kit::array::{F64Array, U8Array};
use xpute_core::sys::errno::Errno;
use xpute_core::sys::error::MarshalError;

/// The section at the front of the core memory that is the module's own.
/// Equal to core_pool.ts's MODULE_BYTES.
pub const MODULE_BYTES: usize = 32 << 20;

extern "C" {
    static __heap_base: u8;
}

/// The module's heap: what is left of the module's section after the
/// linker placed the stack and the data before it (`--stack-first`, the
/// default; 1 MB of stack), up to the section's end. Sized against what a
/// chunk's scratch reaches (wasm_bake.test.ts probes it): about 140 bytes
/// a run at the peak, most of it a vector's old block held while its
/// doubled replacement is filled, so a 65k-run chunk — whose fine mesh no
/// longer fits a pool region — takes under 10 MB of the 31 the heap has.
struct ModuleHeap;

impl xpute_core::ds::fixed_heap::Range for ModuleHeap {
    fn base() -> usize {
        core::ptr::addr_of!(__heap_base) as usize
    }
    const END: usize = MODULE_BYTES;
}

#[global_allocator]
static ALLOC: xpute_core::ds::fixed_heap::FixedHeap<ModuleHeap> = xpute_core::ds::fixed_heap::FixedHeap::new();

struct State {
    /// The submission ring (glue → core) and the completion ring (core →
    /// glue), in the module's heap for as long as the module lives.
    sq: Ring,
    cq: Ring,
    /// The events the builds made that the completion ring had no room for yet.
    events: std::collections::VecDeque<Event>,
    /// Replies the completion ring had no room for yet: tag, command, result
    /// and packet, flushed before any event.
    replies: std::collections::VecDeque<(u32, u32, i32, Option<U8Array>)>,
    /// The block `boot` hands out.
    boot: [u32; BOOT_WORDS],
}

/// Messages each ring holds: a page mounting is 64 chunks a frame, each
/// with its tiers, decor and layout to come, and the glue reads the ring
/// dry between ticks.
pub const RING_CAPACITY: u32 = 4096;

/// Each ring's packet arena: what is left of 1 MiB after the header and the
/// messages. The heap hands out power-of-two classes (fixed_heap.rs), so a
/// ring a byte past 1 MiB would take 2 MiB of it. A chunk's largest message
/// is under 128 bytes, so a full ring's packets fit in about half.
pub const RING_ARENA_BYTES: u32 = (1 << 20) - (RING_HEADER_WORDS + RING_CAPACITY * RING_ENTRY_WORDS) * 4;

fn new_ring(capacity: u32) -> Ring {
    let bytes = ring_bytes(capacity, RING_ARENA_BYTES) as usize;
    // Words of eight bytes, so the ring and every packet in its arena sit on
    // an 8-aligned address, as a packet's f64 and u64 lanes need.
    let words = vec![0u64; bytes.div_ceil(8)].leak();
    // SAFETY: the words are leaked for the module's lifetime and only the
    // two sides of the ring touch them.
    let buffer = unsafe { ArrayBuffer::from_raw(words.as_mut_ptr() as *mut u8, words.len() * 8) };
    Ring::init(buffer, 0, capacity, RING_ARENA_BYTES)
}

/// Where a ring's words start in the memory.
fn ring_address(ring: &Ring) -> u32 {
    ring.words.as_slice().as_ptr() as u32
}

/// The module's one state, made on first use. A cell rather than a
/// `static mut`, whose references the compiler now warns of: the module
/// is single-threaded and no export re-enters another, which is what
/// `state()` promises.
struct Global(core::cell::UnsafeCell<Option<State>>);

// SAFETY: the module runs on one thread; see `state`.
unsafe impl Sync for Global {}

static STATE: Global = Global(core::cell::UnsafeCell::new(None));

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
    // SAFETY: the module runs on one thread and no export re-enters another,
    // so no two references to the state are live at once.
    unsafe {
        (*STATE.0.get()).get_or_insert_with(|| {
            std::panic::set_hook(Box::new(|info| {
                let text = info.to_string();
                host_panic(text.as_ptr(), text.len());
            }));
            State {
                sq: new_ring(RING_CAPACITY),
                cq: new_ring(RING_CAPACITY),
                events: std::collections::VecDeque::new(),
                replies: std::collections::VecDeque::new(),
                boot: [0; BOOT_WORDS],
            }
        })
    }
}

fn input_view((at, count, span): (usize, u32, u32)) -> RunSet<'static> {
    // SAFETY: the client points the bake at a run set in a pool block, which
    // starts 8-aligned and holds run_set_bytes(count, span), and keeps the
    // block until the bake closes.
    unsafe {
        let raw = core::slice::from_raw_parts(at as *const u8, run_set_bytes(count, span) as usize);
        RunSet::carve(raw, count, span)
    }
}

/// Words a band takes in the table the scan reads (a volume of the module's
/// says its bands in, VOLUME_READ): band_z;
/// the addresses of the material, layer and sub-cell lanes (0 for no
/// sub-cells); the extras' count and the addresses of their five lanes.
pub const BAND_WORDS: usize = VOLUME_BAND_WORDS;

fn band_views((at, count): (usize, u32)) -> Vec<BandView<'static>> {
    // SAFETY: the table is either the client's, written into a lane of its
    // own with every address naming a lane of a band it keeps until the scan
    // closes, or the module's own, copied from a volume it holds;
    // ScanContext::begin checks each lane's length against the band's.
    unsafe {
        let table = core::slice::from_raw_parts(at as *const u32, count as usize * BAND_WORDS);
        table
            .chunks_exact(BAND_WORDS)
            .map(|w| {
                let voxels = pxl_core::bake::volume::BAND_VOXELS;
                let extras = w[4] as usize;
                BandView {
                    band_z: w[0] as i32,
                    material: core::slice::from_raw_parts(w[1] as usize as *const u32, voxels),
                    layer_idx: core::slice::from_raw_parts(w[2] as usize as *const u8, voxels),
                    sub_cell: (w[3] != 0).then(|| core::slice::from_raw_parts(w[3] as usize as *const u8, voxels * 4)),
                    extra: ExtraView {
                        col: core::slice::from_raw_parts(w[5] as usize as *const u32, extras),
                        z: core::slice::from_raw_parts(w[6] as usize as *const i32, extras),
                        material: core::slice::from_raw_parts(w[7] as usize as *const u32, extras),
                        layer_idx: core::slice::from_raw_parts(w[8] as usize as *const u8, extras),
                        sub: core::slice::from_raw_parts(w[9] as usize as *const u32, extras),
                    },
                }
            })
            .collect()
    }
}

/// The volumes once VOLUME_POOLS made them; by the field, so the state's
/// other fields stay borrowable beside them.
fn opened(volumes: &mut Option<Volumes>) -> &mut Volumes {
    volumes.as_mut().expect("VOLUME_POOLS before any volume")
}

/// The volumes for a command: EINVAL before VOLUME_POOLS.
fn volumes_of(volumes: &mut Option<Volumes>) -> Result<&mut Volumes, MarshalError> {
    volumes.as_mut().ok_or_else(|| MarshalError::new(Errno::EINVAL, Some("a volume before VOLUME_POOLS"), None))
}

// ---- Objects ----
//
// The client lowers a coordinate before it crosses: the chunk's key (an
// IVec, passed as it is), the band's Z, the band-local lattice position in
// finest-tier steps, and the level. What comes back is lanes to take views
// over, valid until the next write or reindex. The store is the glue's
// (module/world/object/store.ts). Writes, reads and the reference paint
// alike are commands in the rings (pxl_shared::spec::cmd's OBJECT_*).

pub use pxl_core::kernel::{FRAME_WORDS, JOURNAL_CAPACITY, TOPIC_DECOR_COLUMN, TOPIC_OBJECT_CELL, TOPIC_OBJECT_COLUMN, WORLD_TOPICS};

// ---- Rings ----
//
// Commands and their effects cross in rings (crates/xpute/core/src/ipc/ring.rs):
// a message names a command (pxl_shared::spec::cmd) and an XTP packet in the
// ring's arena, which the kernel reads where it lies. The glue assembles
// what it submits and takes apart what completes; nothing here is shaped
// for it. The glue pushes into the submission ring, rings `interrupt`
// (DRAIN or TICK), and reads the completion ring when the call returns.

/// Words in the boot block: the section the module keeps for itself at the
/// front of the memory, which the shell checks its map against before
/// anything else; then where the submission ring and the completion ring
/// are.
pub const BOOT_WORDS: usize = 3;

/// The boot block's address: what the shell reads once, before any ring.
#[no_mangle]
pub extern "C" fn boot() -> *const u32 {
    install_host();
    let s = state();
    let boot = [MODULE_BYTES as u32, ring_address(&s.sq), ring_address(&s.cq)];
    s.boot = boot;
    s.boot.as_ptr()
}

/// The doorbell. The vector says what the turn is for
/// (xpute_core::ipc::cmd::Interrupt): DRAIN applies the submissions waiting
/// and answers how many messages it posted; TICK drains, then runs the
/// scheduler's pass, and answers 1 when the canvas has something new to
/// draw. A vector nobody numbers answers u32::MAX.
#[no_mangle]
pub extern "C" fn interrupt(vector: u32) -> u32 {
    let s = state();
    match vector {
        v if v == Interrupt::DRAIN as u32 => drain(s),
        v if v == Interrupt::TICK as u32 => tick(s),
        _ => u32::MAX,
    }
}

/// A packet of one branch, its children written by `f`.
fn packet(f: &mut dyn FnMut(&mut BranchViewImpl)) -> U8Array {
    let mut t = TreeView::new();
    t.branch(Some(&mut |b| f(b)));
    encoder()
        .encode(
            &t,
            TreeEncoderOptions {
                init_cap: Some(128),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{e}"))
}

/// An event as the signal it raises: its chunk, its command and its packet.
fn signal(e: &Event) -> (u32, Cmd, Option<U8Array>) {
    match *e {
        Event::Build(BuildEvent::Tier {
            id,
            generation,
            step,
            at,
            bytes,
            vertex_cnt,
            index_cnt,
        }) => (
            id,
            Cmd::TIER_CREATE,
            Some(packet(&mut |b| {
                b.u32(Some(generation)).u32(Some(step)).u32(Some(at)).u32(Some(bytes)).u32(Some(vertex_cnt)).u32(Some(index_cnt));
            })),
        ),
        Event::Build(BuildEvent::Runs { id, bytes }) => (
            id,
            Cmd::RUNS_UPDATE,
            Some(packet(&mut |b| {
                b.u32(Some(bytes));
            })),
        ),
        Event::Visible { id, on } => (
            id,
            Cmd::CHUNK_VISIBLE,
            Some(packet(&mut |b| {
                b.bool(Some(on));
            })),
        ),
        Event::Layout { id, primary, secondary, ready } => (
            id,
            Cmd::CHUNK_LAYOUT,
            Some(packet(&mut |b| {
                b.u32(Some(primary)).u32(Some(secondary)).bool(Some(ready));
            })),
        ),
        Event::Blend {
            id,
            primary,
            secondary,
            decor_secondary,
        } => (
            id,
            Cmd::CHUNK_BLEND,
            Some(packet(&mut |b| {
                b.f32(Some(primary)).f32(Some(secondary)).bool(Some(decor_secondary));
            })),
        ),
        Event::Evict { id, step } => (
            id,
            Cmd::TIER_DELETE,
            Some(packet(&mut |b| {
                b.u32(Some(step));
            })),
        ),
        Event::Mount { id, page, base_x, base_y } => (
            id,
            Cmd::CHUNK_CREATE,
            Some(packet(&mut |b| {
                b.u64(Some(page)).u32(Some(base_x)).u32(Some(base_y));
            })),
        ),
        Event::Unmount { id } => (id, Cmd::CHUNK_DELETE, None),
        Event::Decor { id, step, at, bytes } => (
            id,
            Cmd::DECOR_CREATE,
            Some(packet(&mut |b| {
                b.u32(Some(step)).u32(Some(at)).u32(Some(bytes));
            })),
        ),
    }
}

/// Moves what fits of the waiting replies, then of the waiting events, into
/// the completion ring: a reply first, since its call is waiting on it.
fn flush(s: &mut State) -> u32 {
    let mut posted = 0;
    while let Some((tag, number, value, reply)) = s.replies.front() {
        if s.cq.push(*tag, *number, FrameFlag::RES as u32, reply.as_ref(), *value) != Errno::OK {
            return posted;
        }
        s.replies.pop_front();
        posted += 1;
    }
    while let Some(e) = s.events.front() {
        let (tag, cmd, pkt) = signal(e);
        if s.cq.push(tag, cmd as u32, 0, pkt.as_ref(), 0) != Errno::OK {
            break;
        }
        s.events.pop_front();
        posted += 1;
    }
    posted
}

fn now() -> f64 {
    // SAFETY: the import is the client's clock and reads nothing of ours.
    unsafe { host_now() }
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

/// A lane of floats whose length is a whole number of `stride`s — for a
/// theme, exactly `stride`.
fn floats_at(b: &BranchCursor, i: u32, stride: usize) -> Result<xpute_core::kit::array::F32Array, MarshalError> {
    let a = b.at(i)?.get_array::<f32>()?;
    if !a.length().is_multiple_of(stride) {
        return Err(MarshalError::new(
            Errno::EBADMSG,
            Some(&format!("a lane of {} floats is not whole instances of {stride}", a.length())),
            None,
        ));
    }
    Ok(a)
}

/// A u8 lane of exactly `len` elements.
fn bytes_at(b: &BranchCursor, i: u32, len: usize) -> Result<xpute_core::kit::array::U8Array, MarshalError> {
    let a = b.at(i)?.get_array::<u8>()?;
    if len != usize::MAX && a.length() != len {
        return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("a lane of {} bytes, not {len}", a.length())), None));
    }
    Ok(a)
}

fn bool_at(b: &BranchCursor, i: u32) -> Result<bool, MarshalError> {
    b.at(i)?.get_bool()
}

/// A child branch, or none where the packet holds a null.
fn branch_at(b: &BranchCursor, i: u32) -> Result<Option<BranchCursor>, MarshalError> {
    b.at(i)?.opt().map(|c| c.as_branch()).transpose()
}

/// The volume a packet's child `i` names, as its index: 0 is never a volume.
fn volume_at(f: &BranchCursor, i: u32) -> Result<usize, MarshalError> {
    match u32_at(f, i)? {
        0 => Err(MarshalError::new(Errno::EINVAL, Some("volume 0"), None)),
        id => Ok(id as usize - 1),
    }
}

/// The pool a packet's first child names.
fn pool_at<'a>(k: &'a mut Kernel, f: &BranchCursor) -> Result<&'a mut Pool, MarshalError> {
    let id = u32_at(f, 0)?;
    k.pools.get_mut(id as usize).ok_or_else(|| MarshalError::new(Errno::EINVAL, Some(&format!("no pool {id}")), None))
}

/// A ray as a query's packet carries it: the map origin, the origin, the
/// direction and the distance.
fn ray_at(f: &BranchCursor) -> Result<[f64; 9], MarshalError> {
    let r = f.at(0)?.get_array::<f64>()?;
    <[f64; 9]>::try_from(r.as_slice()).map_err(|_| MarshalError::new(Errno::EBADMSG, Some(&format!("a ray of {} doubles, not 9", r.length())), None))
}

/// Applies one submission: its reply's value and packet, or none for a
/// command nobody numbers, which replies ENOSYS; a packet that does not read
/// as the command's replies with its errno.
fn apply(s: &mut State, tag: u32, number: u32, pkt: &Option<U8Array>) -> Result<Option<(i32, Option<U8Array>)>, MarshalError> {
    let k = kernel();
    let Some(cmd) = Cmd::of(number) else { return Ok(None) };
    match cmd {
        Cmd::OBJECT_UPDATE => {
            let f = fields(pkt)?;
            k.objects
                .write(u64_at(&f, 0)?, i32_at(&f, 1)?, [u32_at(&f, 2)?, u32_at(&f, 3)?, u32_at(&f, 4)?], i32_at(&f, 5)?, u32_at(&f, 6)?);
            return Ok(Some((0, None)));
        }
        Cmd::OBJECT_DELETE => {
            k.objects.clear();
            return Ok(Some((0, None)));
        }
        Cmd::OBJECT_REINDEX => {
            let key = u64_at(&fields(pkt)?, 0)?;
            reindex_object(k, key);
            return Ok(Some((0, None)));
        }
        Cmd::STATE_UPDATE => {
            let f = fields(pkt)?;
            let kind = match u32_at(&f, 1)? {
                1 => Kind::Solid,
                2 => Kind::Air,
                3 => Kind::Decor,
                _ => Kind::Untaught,
            };
            k.painter.teach(u32_at(&f, 0)?, kind, u32_at(&f, 2)?, u32_at(&f, 3)? as u8);
            return Ok(Some((0, None)));
        }
        Cmd::TERRAIN_UPDATE => {
            let f = fields(pkt)?;
            let base = f.at(0)?.as_branch()?;
            let channels = f.at(1)?.as_branch()?;
            let t = &mut k.terrain;
            t.base_rgb = [u32_at(&base, 0)? as u8, u32_at(&base, 1)? as u8, u32_at(&base, 2)? as u8];
            t.base_layer = u32_at(&base, 3)? as u8;
            t.base_class = u32_at(&base, 4)? as u8;
            for (ch, dst) in t.channels.iter_mut().enumerate() {
                let c = channels.at(ch as u32)?.as_branch()?;
                let (rgb, has_color, layer, has_layer) = (bytes_at(&c, 1, 768)?, bytes_at(&c, 2, 256)?, bytes_at(&c, 3, 256)?, bytes_at(&c, 4, 256)?);
                dst.class = u32_at(&c, 0)? as u8;
                for k in 0..256 {
                    dst.rgb[k] = [rgb[k * 3], rgb[k * 3 + 1], rgb[k * 3 + 2]];
                    dst.has_color[k] = has_color[k] != 0;
                    dst.layer[k] = layer[k];
                    dst.has_layer[k] = has_layer[k] != 0;
                }
            }
            return Ok(Some((0, None)));
        }
        Cmd::DECOR_KIND_UPDATE => {
            let kinds = fields(pkt)?.at(0)?.as_branch()?;
            k.sched.decor_forget();
            for kd in kinds.iter() {
                let kd = kd?.as_branch()?;
                let value = i32_at(&kd, 5)?;
                let idx = k.sched.decor_kind(DecorKind {
                    tag: u64_at(&kd, 0)?,
                    density: number_at(&kd, 1)?,
                    variant_count: u32_at(&kd, 2)?,
                    align_transport: bool_at(&kd, 3)?,
                    extent: number_at(&kd, 4)?,
                    decor_value: (value >= 0).then_some(value as u32),
                    rules: [DecorRule::Any; 4],
                });
                let rules = kd.at(6)?.as_branch()?;
                for ch in 0..4 {
                    let r = rules.at(ch)?.as_branch()?;
                    match u32_at(&r, 0)? {
                        0 => {}
                        1 => k.sched.decor_rule(idx, ch as usize, DecorRule::Void),
                        _ => {
                            k.sched.decor_rule(idx, ch as usize, DecorRule::OneOf([0; 4]));
                            for &t in bytes_at(&r, 1, usize::MAX)?.as_slice() {
                                k.sched.decor_allow(idx, ch as usize, t);
                            }
                        }
                    }
                }
            }
            return Ok(Some((0, None)));
        }
        Cmd::DECOR_KIND_MODEL => {
            let f = fields(pkt)?;
            k.sched.decor_model(u32_at(&f, 0)?, u32_at(&f, 1)?);
            return Ok(Some((0, None)));
        }
        Cmd::CHANGE_UPDATE | Cmd::CHANGE_READ | Cmd::CHANGE_DELETE => {
            let f = fields(pkt)?;
            let (topic, key) = (u32_at(&f, 0)?, u64_at(&f, 1)? as i64);
            let revision = match cmd {
                Cmd::CHANGE_UPDATE => k.changes.bump(topic, key),
                Cmd::CHANGE_READ => k.changes.revision_of(topic, key),
                _ => {
                    k.changes.forget(topic, key);
                    0
                }
            };
            return Ok(Some((revision as i32, None)));
        }
        Cmd::FRAME_READ => {
            let mut uniform = [pxl_core::render::shader::FRAME_FLOATS; 14];
            uniform[1..].copy_from_slice(&pxl_core::render::shader::FRAME_AT);
            let (frame_at, uniform) = (k.frame.as_ptr() as u32, xpute_core::kit::array::U32Array::from(&uniform));
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u32(Some(frame_at)).u32_array(Some(&uniform));
                })),
            )));
        }
        Cmd::CHUNK_READ => {
            let (census_at, stat_at) = (k.census.as_ptr() as u32, k.build_stat.as_ptr() as u32);
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u32(Some(census_at)).u32(Some(stat_at));
                })),
            )));
        }
        Cmd::ENV_LANES => {
            let out_at = k.sched.environment.out.as_ptr() as u32;
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u32(Some(out_at));
                })),
            )));
        }
        Cmd::CHANGE_LANES => {
            let l = k.changes.lanes();
            let capacity = k.changes.capacity;
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u32(Some(l[0] as u32))
                        .u32(Some(l[1] as u32))
                        .u32(Some(l[2] as u32))
                        .u32(Some(l[3] as u32))
                        .u32(Some(l[4] as u32))
                        .u32(Some(capacity));
                })),
            )));
        }
        Cmd::DRAW_READ => {
            let lanes = k.sched.draw.take();
            let row = xpute_core::kit::array::U32Array::from(&lanes.map(|v| v as u32));
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u32_array(Some(&row));
                })),
            )));
        }
        Cmd::SHADER_READ => {
            let src = pxl_core::render::shader::source(u32_at(&fields(pkt)?, 0)?);
            let at = src.as_ptr() as u32;
            return Ok(Some((
                src.len() as i32,
                Some(packet(&mut |b| {
                    b.u32(Some(at));
                })),
            )));
        }
        Cmd::SHADER_PIPELINE => {
            let record = xpute_core::kit::array::U32Array::from(&pxl_core::render::shader::pipeline(u32_at(&fields(pkt)?, 0)?));
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u32_array(Some(&record));
                })),
            )));
        }
        Cmd::MODEL_SEAL => {
            let scale = number_at(&fields(pkt)?, 0)? as f32;
            let mem = opened(&mut k.volumes).mem();
            let sched = &mut k.sched;
            // SAFETY: the model's block is in the volumes' memory.
            unsafe { sched.models.seal(&mut sched.draw, mem, tag, scale) };
            return Ok(Some((0, None)));
        }
        Cmd::OBJECT_READ => {
            let f = fields(pkt)?;
            let state = k.objects.read(u64_at(&f, 0)?, i32_at(&f, 1)?, [u32_at(&f, 2)?, u32_at(&f, 3)?, u32_at(&f, 4)?]);
            return Ok(Some((state as i32, None)));
        }
        Cmd::OBJECT_LANES => {
            let key = u64_at(&fields(pkt)?, 0)?;
            let Some(lanes) = k.objects.chunk_lanes(key) else { return Ok(Some((0, None))) };
            let reply = packet(&mut |b| {
                b.u32(Some(lanes.band_at as u32)).u32(Some(lanes.pos_at as u32)).u32(Some(lanes.state_at as u32));
            });
            return Ok(Some((lanes.count as i32, Some(reply))));
        }
        Cmd::OBJECT_CELL => {
            let f = fields(pkt)?;
            let Some(rec) = k.objects.cell(u64_at(&f, 0)?, u32_at(&f, 1)?, u32_at(&f, 2)?, i32_at(&f, 3)?) else {
                return Ok(Some((0, None)));
            };
            return Ok(Some((
                1,
                Some(packet(&mut |b| {
                    b.u32(Some(rec.start)).u32(Some(rec.count)).u32(Some(rec.solid));
                })),
            )));
        }
        Cmd::OBJECT_COLUMNS => {
            let keys: Vec<u64> = (0..k.objects.chunk_count()).map(|i| k.objects.chunk_key(i)).collect();
            let keys = xpute_core::kit::array::U64Array::from(&keys);
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u64_array(Some(&keys));
                })),
            )));
        }
        Cmd::OBJECT_STAT => {
            let o = &k.objects;
            let (chunks, bands, cells) = (o.chunk_count() as u32, o.band_count() as u32, o.cell_count() as u32);
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u32(Some(chunks)).u32(Some(bands)).u32(Some(cells));
                })),
            )));
        }
        Cmd::POOL_CREATE => {
            let f = fields(pkt)?;
            let (start, bytes) = (u32_at(&f, 0)? as usize, u32_at(&f, 1)? as usize);
            // SAFETY: the memory is the module's own linear memory, addressed
            // from 0, and the shell keeps the range for this pool alone.
            let pool = unsafe { Pool::new(core::ptr::null_mut(), start + bytes, start, bytes, u32_at(&f, 2)?, u32_at(&f, 3)?) };
            k.pools.push(pool);
            return Ok(Some((k.pools.len() as i32 - 1, None)));
        }
        Cmd::POOL_READ => {
            let f = fields(pkt)?;
            return Ok(Some((pool_at(k, &f)?.stat_words() as usize as i32, None)));
        }
        Cmd::POOL_ACQUIRE => {
            let f = fields(pkt)?;
            let bytes = u32_at(&f, 1)?;
            return Ok(Some((pool_at(k, &f)?.acquire(bytes).map_or(0, |at| at as i32), None)));
        }
        Cmd::POOL_RELEASE => {
            let f = fields(pkt)?;
            let (at, bytes) = (u32_at(&f, 1)? as usize, u32_at(&f, 2)?);
            if !pool_at(k, &f)?.release(at, bytes) {
                return Err(MarshalError::new(
                    Errno::EINVAL,
                    Some(&format!("a {bytes}B block at {at} its pool does not hold, or holds already")),
                    None,
                ));
            }
            return Ok(Some((0, None)));
        }
        Cmd::POOL_FREE => {
            let f = fields(pkt)?;
            let (at, bytes) = (u32_at(&f, 1)? as usize, u32_at(&f, 2)?);
            return Ok(Some((pool_at(k, &f)?.is_free(at, bytes) as i32, None)));
        }
        Cmd::VOLUME_POOLS => {
            let f = fields(pkt)?;
            // SAFETY: the pools were cut from this module's memory, addressed from 0.
            k.volumes = Some(unsafe { Volumes::new(core::ptr::null_mut(), u32_at(&f, 0)?, u32_at(&f, 1)?) });
            return Ok(Some((0, None)));
        }
        Cmd::VOLUME_STAT => {
            let at = &volumes_of(&mut k.volumes)?.stat as *const _ as usize;
            return Ok(Some((at as i32, None)));
        }
        Cmd::VOLUME_CREATE => return Ok(Some((volumes_of(&mut k.volumes)?.open() as i32 + 1, None))),
        Cmd::VOLUME_READ => {
            let id = volume_at(&fields(pkt)?, 0)?;
            let v = volumes_of(&mut k.volumes)?;
            let count = v.band_count(id);
            if count == 0 {
                return Ok(Some((0, None)));
            }
            // The words, not the row's address: a row is written into one
            // scratch the volumes keep, so every band's address would be the
            // last band's.
            let rows: Vec<u32> = (0..count).flat_map(|i| *v.band_row(id, i)).collect();
            let rows = xpute_core::kit::array::U32Array::from(&rows);
            return Ok(Some((
                count as i32,
                Some(packet(&mut |b| {
                    b.u32_array(Some(&rows));
                })),
            )));
        }
        Cmd::VOLUME_DELETE => {
            let id = volume_at(&fields(pkt)?, 0)?;
            volumes_of(&mut k.volumes)?.release(&mut k.pools, id);
            return Ok(Some((0, None)));
        }
        Cmd::VOLUME_FILL => {
            let f = fields(pkt)?;
            let id = volume_at(&f, 0)?;
            let voxel = Voxel {
                material: u32_at(&f, 5)?,
                layer_idx: u32_at(&f, 6)? as u8,
                sub: 0,
            };
            let (cx, cy, z0, z1) = (u32_at(&f, 1)?, u32_at(&f, 2)?, i32_at(&f, 3)?, i32_at(&f, 4)?);
            let ok = volumes_of(&mut k.volumes)?.fill_column(&mut k.pools, id, (cx, cy), z0, z1, voxel);
            return Ok(Some((ok as i32, None)));
        }
        Cmd::VOLUME_VOXEL | Cmd::VOLUME_SUB_VOXEL => {
            let f = fields(pkt)?;
            let id = volume_at(&f, 0)?;
            let cell = Cell {
                cx: u32_at(&f, 1)?,
                cy: u32_at(&f, 2)?,
                z: i32_at(&f, 3)?,
            };
            let voxel = Voxel {
                material: u32_at(&f, 4)?,
                layer_idx: u32_at(&f, 5)? as u8,
                sub: u32_at(&f, 6)?,
            };
            let v = volumes_of(&mut k.volumes)?;
            let ok = if cmd == Cmd::VOLUME_VOXEL {
                v.set_voxel(&mut k.pools, id, cell, voxel)
            } else {
                v.append_sub_voxel(&mut k.pools, id, cell, voxel)
            };
            return Ok(Some((ok as i32, None)));
        }
        Cmd::VOLUME_SEAL => {
            let id = volume_at(&fields(pkt)?, 0)?;
            volumes_of(&mut k.volumes)?.seal(id);
            return Ok(Some((0, None)));
        }
        Cmd::BAKE_CREATE => {
            if k.ctx.is_open() {
                return Ok(Some((0, None)));
            }
            let f = fields(pkt)?;
            k.input = (u32_at(&f, 0)? as usize, u32_at(&f, 1)?, u32_at(&f, 2)?);
            let lod_step = u32_at(&f, 3)?;
            return Ok(Some((k.ctx.begin(&input_view(k.input), lod_step) as i32, None)));
        }
        Cmd::BAKE_STEP => {
            return Ok(Some(match k.ctx.step(&input_view(k.input)) {
                Step::Working => (StepResult::WORKING as i32, None),
                Step::NeedMesh(words) => (
                    StepResult::NEED_BLOCK as i32,
                    Some(packet(&mut |b| {
                        b.u32(Some(words));
                    })),
                ),
                Step::Done => (StepResult::DONE as i32, None),
            }));
        }
        Cmd::BAKE_MESH => {
            let f = fields(pkt)?;
            let (at, words) = (u32_at(&f, 0)? as usize, u32_at(&f, 1)?);
            // SAFETY: the shell's block, `words` words at `at`, 4-aligned, kept
            // until the bake finishes or aborts.
            unsafe { k.ctx.provide_mesh(at as *mut u32, words) }
            return Ok(Some((0, None)));
        }
        Cmd::BAKE_FINISH => {
            let mesh = k.ctx.finish();
            let c = k.ctx.counters;
            k.result = [
                mesh.vertex_cnt,
                mesh.index_cnt,
                c.total_runs,
                c.culled,
                c.faces_possible,
                c.faces_kept,
                c.coarse_total,
                c.coarse_low_fill,
                ALLOC.used() as u32,
            ];
            return Ok(Some((k.result.as_ptr() as usize as i32, None)));
        }
        Cmd::BAKE_DELETE => {
            k.ctx.abort();
            return Ok(Some((0, None)));
        }
        Cmd::SCAN_CREATE => {
            if k.scan.is_open() {
                return Ok(Some((0, None)));
            }
            let id = volume_at(&fields(pkt)?, 0)?;
            let v = volumes_of(&mut k.volumes)?;
            let count = v.band_count(id);
            // The rows copied out, so the scan reads them as a table of its own.
            k.scan_rows.clear();
            for i in 0..count {
                k.scan_rows.push(*v.band_row(id, i));
            }
            k.bands = (k.scan_rows.as_ptr() as usize, count as u32);
            return Ok(Some((k.scan.begin(&band_views(k.bands)) as i32, None)));
        }
        Cmd::SCAN_STEP => {
            return Ok(Some(match k.scan.step(&band_views(k.bands)) {
                ScanStep::Working => (StepResult::WORKING as i32, None),
                ScanStep::NeedSet(bytes) => (
                    StepResult::NEED_BLOCK as i32,
                    Some(packet(&mut |b| {
                        b.u32(Some(bytes));
                    })),
                ),
                ScanStep::Done => (StepResult::DONE as i32, None),
            }));
        }
        Cmd::SCAN_SET => {
            let f = fields(pkt)?;
            let (at, bytes) = (u32_at(&f, 0)? as usize, u32_at(&f, 1)?);
            // SAFETY: the shell's block, `bytes` bytes at `at`, 4-aligned.
            unsafe { k.scan.provide_set(at as *mut u8, bytes) }
            return Ok(Some((k.scan.finish() as i32, None)));
        }
        Cmd::SCAN_DELETE => {
            k.scan.abort();
            return Ok(Some((0, None)));
        }
        Cmd::TERRAIN_READ => {
            let f = fields(pkt)?;
            let (tx0, ty0) = (u32_at(&f, 5)?, u32_at(&f, 6)?);
            if tx0 > PAGE_CELL_SPAN - CHUNK_SPAN || ty0 > PAGE_CELL_SPAN - CHUNK_SPAN {
                return Err(MarshalError::new(Errno::EINVAL, Some("a chunk past its page"), None));
            }
            let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
            let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
            let lanes = [u32_at(&f, 0)?, u32_at(&f, 1)?, u32_at(&f, 2)?, u32_at(&f, 3)?, u32_at(&f, 4)?].map(|at| at as usize);
            // SAFETY: the shell's page lanes, kept in the core memory for as long
            // as the page is resident; a page's shape is fixed.
            let page = unsafe {
                PageView {
                    ground: [
                        core::slice::from_raw_parts(lanes[0] as *const u8, cells),
                        core::slice::from_raw_parts(lanes[1] as *const u8, cells),
                        core::slice::from_raw_parts(lanes[2] as *const u8, cells),
                        core::slice::from_raw_parts(lanes[3] as *const u8, cells),
                    ],
                    ele: core::slice::from_raw_parts(lanes[4] as *const i16, samples),
                }
            };
            k.chunk.read(&page, &k.terrain, tx0, ty0, bool_at(&f, 7)?);
            let c = &k.chunk;
            let (floor, material_at, layer_at, ele_at) = (c.floor, c.material.as_ptr() as usize as u32, c.layer_idx.as_ptr() as usize as u32, c.ele.as_ptr() as usize as u32);
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.i32(Some(floor)).u32(Some(material_at)).u32(Some(layer_at)).u32(Some(ele_at));
                })),
            )));
        }
        Cmd::TERRAIN_PAINT => {
            let id = volume_at(&fields(pkt)?, 0)?;
            let v = volumes_of(&mut k.volumes)?;
            return Ok(Some((k.chunk.paint(v, &mut k.pools, id) as i32, None)));
        }
        Cmd::OBJECT_PAINT => {
            let f = fields(pkt)?;
            let (id, key) = (volume_at(&f, 0)?, u64_at(&f, 1)?);
            let hide = match branch_at(&f, 2)? {
                Some(h) => Some((i32_at(&h, 0)?, u32_at(&h, 1)?)),
                None => None,
            };
            let carve = match branch_at(&f, 3)? {
                Some(c) => Some(Carve {
                    lx: number_at(&c, 0)?,
                    ly: number_at(&c, 1)?,
                    lz: number_at(&c, 2)?,
                    size: number_at(&c, 3)?,
                }),
                None => None,
            };
            let v = volumes_of(&mut k.volumes)?;
            let made = k.painter.paint(&k.objects, v, &mut k.pools, id, key, hide, carve);
            return Ok(Some(((made.any as u32 | ((made.refused as u32) << 1)) as i32, None)));
        }
        Cmd::RAY_COLUMN => {
            let f = fields(pkt)?;
            let id = volume_at(&f, 0)?;
            let (cx, cy) = (u32_at(&f, 1)?, u32_at(&f, 2)?);
            let v = volumes_of(&mut k.volumes)?;
            let rows: Vec<[u32; VOLUME_BAND_WORDS]> = (0..v.band_count(id)).map(|i| *v.band_row(id, i)).collect();
            let n = k.column.walk(&band_views((rows.as_ptr() as usize, rows.len() as u32)), cx, cy);
            let c = &k.column;
            let lanes = [
                c.z0.as_ptr() as usize as u32,
                c.z1.as_ptr() as usize as u32,
                c.material_class.as_ptr() as usize as u32,
                c.sub.as_ptr() as usize as u32,
            ];
            return Ok(Some((
                n as i32,
                Some(packet(&mut |b| {
                    for at in lanes {
                        b.u32(Some(at));
                    }
                })),
            )));
        }
        Cmd::WORLD_UPDATE => {
            let f = fields(pkt)?;
            let platform = pxl_core::module::world::world::Platform {
                geolocation: bool_at(&f, 4)?,
                orientation: pxl_core::module::device::pose::use_device_orientation::OrientationPlatform {
                    supported: bool_at(&f, 5)?,
                    needs_permission: bool_at(&f, 6)?,
                },
            };
            let asked = pxl_core::module::world::world::world_frame(number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?, number_at(&f, 3)?, platform);
            return Ok(Some((asked as i32, None)));
        }
        Cmd::WORLD_READ => {
            use pxl_core::module::device::pose::use_device_orientation::{take_orientation_commands, OrientationCommand};
            use pxl_core::module::device::pose::use_geolocation::{take_geolocation_commands, GeolocationCommand};
            use pxl_core::module::map::ui::toast::toast_controller::{take_toast_commands, ToastCommand, ToastTone};
            let toasts = take_toast_commands();
            let geo: Vec<u32> = take_geolocation_commands()
                .into_iter()
                .map(|c| match c {
                    GeolocationCommand::Start => 0,
                    GeolocationCommand::Stop => 1,
                })
                .collect();
            let ori: Vec<u32> = take_orientation_commands()
                .into_iter()
                .map(|c| match c {
                    OrientationCommand::Listen => 0,
                    OrientationCommand::RequestPermission => 1,
                    OrientationCommand::Remove => 2,
                })
                .collect();
            let log = pxl_core::module::world::sched::log::take_log_lines();
            let draw = pxl_core::module::world::core_tick::take_draw();
            let ui = pxl_core::app::shell::ui_snapshot_json();
            let mut decor: Vec<(u32, u32, f64)> = Vec::new();
            let mut group = 0u32;
            for entry in pxl_core::module::world::decor::asset_store::decor_bundle_assets() {
                for (v, model) in entry.models.iter().enumerate() {
                    let mesh = model.as_ref().and_then(|m| k.sched.models.mesh(m.model)).map_or(0, |mesh| mesh.handle);
                    let skin = entry.skins.get(v).copied().flatten().map_or(-1.0, |s| s as f64);
                    decor.push((group, mesh, skin));
                    group += 1;
                }
            }
            let skins = pxl_core::module::world::character::companion::companion_skins();
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.branch(Some(&mut |t| {
                        for c in &toasts {
                            t.branch(Some(&mut |e| match c {
                                ToastCommand::Show(spec) => {
                                    let tone = match spec.tone {
                                        ToastTone::Loading => 0,
                                        ToastTone::Error => 1,
                                        ToastTone::Success => 2,
                                    };
                                    e.bool(Some(true)).str(Some(spec.id)).str(Some(spec.message_key.as_str())).u32(Some(tone));
                                }
                                ToastCommand::Dismiss(id) => {
                                    e.bool(Some(false)).str(Some(id)).str(Some("")).u32(Some(0));
                                }
                            }));
                        }
                    }));
                    b.branch(Some(&mut |g| {
                        for c in &geo {
                            g.u32(Some(*c));
                        }
                    }));
                    b.branch(Some(&mut |o| {
                        for c in &ori {
                            o.u32(Some(*c));
                        }
                    }));
                    b.branch(Some(&mut |l| {
                        for line in &log {
                            l.str(Some(line.as_str()));
                        }
                    }));
                    b.bool(Some(draw));
                    b.str(Some(ui.as_str()));
                    b.branch(Some(&mut |d| {
                        for (group, mesh, skin) in &decor {
                            d.branch(Some(&mut |e| {
                                e.u32(Some(*group)).u32(Some(*mesh)).f64(Some(*skin));
                            }));
                        }
                    }));
                    b.branch(Some(&mut |sk| {
                        for (model, skin) in &skins {
                            sk.branch(Some(&mut |e| {
                                e.u32(Some(*model)).u32(Some(*skin));
                            }));
                        }
                    }));
                })),
            )));
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
            pxl_core::host::set_handheld(bool_at(&f, 5)?);
            pxl_core::module::world::terrain::config::set_page_section_bytes(u32_at(&f, 6)?);
            pxl_core::module::world::chunk_node::set_mesh_section_bytes(number_at(&f, 7)?);
            pxl_core::module::world::object::store::load();
            return Ok(Some((0, None)));
        }
        Cmd::WORLD_LANES => {
            let at = pxl_core::render::gpu::stage::stage_address() as u32;
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.u32(Some(at));
                })),
            )));
        }
        Cmd::WORLD_ACTION => {
            let applied = pxl_core::app::shell::ui_action(&fields(pkt)?.at(0)?.get_str()?);
            return Ok(Some((applied as i32, None)));
        }
        Cmd::WORLD_SHEET => {
            use pxl_core::module::world::billboard::atlas::DrawSpec;
            let generation = u32_at(&fields(pkt)?, 0)?;
            let atlas = pxl_core::module::world::billboard::sheet::billboard_sheet();
            let flush = atlas.flush(generation);
            let slots = if flush.whole { atlas.live() } else { flush.dirty.clone() };
            let rows: Vec<_> = slots
                .iter()
                .filter_map(|&slot| {
                    let rect = atlas.slot_rect(slot);
                    atlas.slot_draw(slot).map(|draw| match draw {
                        DrawSpec::PoiBadge { icon, color, name, logical_w } => (rect, 0u32, icon.to_string(), color.to_string(), name.clone(), *logical_w, 0.0),
                        DrawSpec::Cluster { label, logical_w } => (rect, 1u32, String::new(), String::new(), label.clone(), *logical_w, 0.0),
                        DrawSpec::Bubble { lines, box_w, box_h } => (rect, 2u32, String::new(), String::new(), lines.join("\n"), *box_w, *box_h),
                    })
                })
                .collect();
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.bool(Some(flush.whole)).u32(Some(flush.size)).u32(Some(flush.generation));
                    b.branch(Some(&mut |e| {
                        for (rect, kind, icon, color, text, logical_w, box_h) in &rows {
                            e.branch(Some(&mut |r| {
                                r.u32(Some(rect[0])).u32(Some(rect[1])).u32(Some(rect[2])).u32(Some(rect[3])).u32(Some(*kind));
                                r.str(Some(icon.as_str())).str(Some(color.as_str())).str(Some(text.as_str())).f64(Some(*logical_w)).f64(Some(*box_h));
                            }));
                        }
                    }));
                })),
            )));
        }
        Cmd::INPUT_UPDATE => {
            let f = fields(pkt)?;
            pxl_core::module::camera::input::camera_input_rect(pxl_core::module::camera::input::Rect {
                left: number_at(&f, 0)?,
                top: number_at(&f, 1)?,
                width: number_at(&f, 2)?,
                height: number_at(&f, 3)?,
            });
            return Ok(Some((0, None)));
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
            return Ok(Some((0, None)));
        }
        Cmd::INPUT_WHEEL => {
            pxl_core::module::camera::input::camera_wheel(number_at(&fields(pkt)?, 0)?);
            return Ok(Some((0, None)));
        }
        Cmd::SENSOR_UPDATE => {
            let f = fields(pkt)?;
            let opt = |v: f64| (!v.is_nan()).then_some(v);
            pxl_core::module::device::pose::use_geolocation::geolocation_fix(number_at(&f, 0)?, number_at(&f, 1)?, opt(number_at(&f, 2)?), number_at(&f, 3)?, opt(number_at(&f, 4)?));
            return Ok(Some((0, None)));
        }
        Cmd::SENSOR_ERROR => {
            use pxl_core::module::device::pose::use_geolocation::{geolocation_error, GeolocationErrorCode};
            let code = match u32_at(&fields(pkt)?, 0)? {
                1 => GeolocationErrorCode::PermissionDenied,
                2 => GeolocationErrorCode::PositionUnavailable,
                3 => GeolocationErrorCode::Timeout,
                _ => GeolocationErrorCode::Other,
            };
            geolocation_error(code);
            return Ok(Some((0, None)));
        }
        Cmd::SENSOR_ORIENTATION => {
            let f = fields(pkt)?;
            let opt = |v: f64| (!v.is_nan()).then_some(v);
            pxl_core::module::device::pose::use_device_orientation::orientation_event(opt(number_at(&f, 0)?), opt(number_at(&f, 1)?), opt(number_at(&f, 2)?), opt(number_at(&f, 3)?));
            return Ok(Some((0, None)));
        }
        Cmd::SENSOR_PERMISSION => {
            let f = fields(pkt)?;
            let (granted, failed) = (bool_at(&f, 1)?, bool_at(&f, 2)?);
            let platform = pxl_core::module::world::world::world_platform();
            match (u32_at(&f, 0)?, granted) {
                (0, true) => pxl_core::module::device::pose::use_geolocation::geolocation_permission_granted(platform.geolocation),
                (0, false) => {}
                _ => pxl_core::module::device::pose::use_device_orientation::orientation_permission(granted, failed),
            }
            return Ok(Some((0, None)));
        }
        Cmd::SENSOR_VISIBILITY => {
            let visible = bool_at(&fields(pkt)?, 0)?;
            let platform = pxl_core::module::world::world::world_platform();
            pxl_core::module::device::pose::use_geolocation::geolocation_visibility(visible, platform.geolocation);
            if visible {
                pxl_core::module::device::pose::use_device_orientation::orientation_visible();
            }
            return Ok(Some((0, None)));
        }
        Cmd::PAGE_TERRAIN => {
            use pxl_core::module::world::terrain::tile_repository::{open_terrain, TerrainLanes};
            let f = fields(pkt)?;
            let page = u64_at(&f, 0)?;
            let lanes = TerrainLanes {
                ground: [u32_at(&f, 1)? as usize, u32_at(&f, 2)? as usize, u32_at(&f, 3)? as usize, u32_at(&f, 4)? as usize],
                ele: u32_at(&f, 5)? as usize,
            };
            let mut blocks = Vec::new();
            for b in f.at(6)?.as_branch()?.iter() {
                let b = b?.as_branch()?;
                blocks.push((u32_at(&b, 0)? as usize, u32_at(&b, 1)?));
            }
            // SAFETY: the shell copied the page's lanes into blocks of the page
            // section and holds them for as long as the page is resident.
            let terrain = unsafe { open_terrain(lanes) };
            pxl_core::module::world::terrain::resident::into_core(page, blocks, bool_at(&f, 7)?);
            pxl_core::module::world::terrain::store::use_tile_store().set_tile(page, pxl_shared::bake::format::Tile { terrain });
            return Ok(Some((0, None)));
        }
        Cmd::PAGE_POI => {
            use pxl_core::module::world::poi::repository::{empty_page, open_poi_page};
            let f = fields(pkt)?;
            let page = u64_at(&f, 0)?;
            let resident = match branch_at(&f, 1)? {
                // SAFETY: the shell copied the packet into this block of the page
                // section and holds it for this page alone until it is released.
                Some(b) => unsafe { open_poi_page(u32_at(&b, 0)? as usize, u32_at(&b, 1)?)? },
                None => empty_page()?,
            };
            pxl_core::module::world::poi::store::set_pois(page, resident);
            return Ok(Some((0, None)));
        }
        Cmd::FETCH_READ => {
            use pxl_core::module::world::sched::fetch_scheduler::{fetch_take_aborts, fetch_take_started};
            let started = fetch_take_started();
            let aborts = fetch_take_aborts();
            let mut releases = pxl_core::module::world::terrain::resident::take_due_releases();
            releases.extend(pxl_core::module::world::poi::store::take_released_blocks());
            let rows: Vec<(String, String, String)> = started
                .into_iter()
                .map(|d| {
                    let page = d.key.split_once(':').map_or(d.key.as_str(), |(_, p)| p);
                    let url = match d.category.as_str() {
                        "terrain" => pxl_core::module::world::terrain::tile_repository::tile_url(page),
                        "poi" => pxl_core::module::world::poi::repository::poi_url(page),
                        "geocode" => pxl_core::module::world::geocode::repository::geocode_url(page),
                        "search" => pxl_core::module::map::ui::overlay::search_modal::search_request_url(page),
                        _ => String::new(),
                    };
                    (d.key.clone(), d.category.clone(), url)
                })
                .collect();
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.branch(Some(&mut |s| {
                        for (key, category, url) in &rows {
                            s.branch(Some(&mut |r| {
                                r.str(Some(key.as_str())).str(Some(category.as_str())).str(Some(url.as_str()));
                            }));
                        }
                    }));
                    b.branch(Some(&mut |a| {
                        for key in &aborts {
                            a.str(Some(key.as_str()));
                        }
                    }));
                    b.branch(Some(&mut |r| {
                        for (at, bytes) in &releases {
                            r.branch(Some(&mut |e| {
                                e.u32(Some(*at as u32)).u32(Some(*bytes));
                            }));
                        }
                    }));
                })),
            )));
        }
        Cmd::FETCH_UPDATE => {
            use pxl_core::module::world::sched::ledger::Outcome;
            let f = fields(pkt)?;
            let key = f.at(0)?.get_str()?;
            let outcome = match u32_at(&f, 1)? {
                0 => Outcome::Ok,
                1 => Outcome::Aborted,
                _ => Outcome::Error,
            };
            let error = match f.at(2)?.opt() {
                Some(c) => Some(c.get_str()?),
                None => None,
            };
            let mut phases = Vec::new();
            for p in f.at(3)?.as_branch()?.iter() {
                let p = p?.as_branch()?;
                phases.push((p.at(0)?.get_str()?, number_at(&p, 1)?));
            }
            if let (false, Some(query)) = (outcome == Outcome::Ok, key.strip_prefix("search:")) {
                pxl_core::module::map::ui::overlay::search_modal::search_failed(query, outcome == Outcome::Aborted);
            }
            if let (false, Some(page)) = (outcome == Outcome::Ok, key.strip_prefix("geocode:").and_then(|p| p.parse::<u64>().ok())) {
                pxl_core::module::map::ui::overlay::use_reverse_geocode::reverse_geocode_failed(page, outcome == Outcome::Aborted);
            }
            pxl_core::module::world::sched::fetch_scheduler::fetch_settle(&key, outcome, (!phases.is_empty()).then_some(phases), error);
            return Ok(Some((0, None)));
        }
        Cmd::FETCH_GEOCODE => {
            let f = fields(pkt)?;
            let page = u64_at(&f, 0)?;
            let body = match f.at(1)?.opt() {
                Some(c) => Some(c.get_str()?),
                None => None,
            };
            pxl_core::module::map::ui::overlay::use_reverse_geocode::reverse_geocode_arrived(page, body.as_deref());
            return Ok(Some((0, None)));
        }
        Cmd::FETCH_SEARCH => {
            let f = fields(pkt)?;
            let query = f.at(0)?.get_str()?;
            let body = match f.at(1)?.opt() {
                Some(c) => Some(c.get_str()?),
                None => None,
            };
            pxl_core::module::map::ui::overlay::search_modal::search_arrived(&query, body.as_deref());
            return Ok(Some((0, None)));
        }
        Cmd::LOAD_READ => {
            use pxl_core::render::gltf::load::{take_load_requests, LoadKind};
            let requests = take_load_requests();
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.branch(Some(&mut |r| {
                        for q in &requests {
                            let kind = match q.kind {
                                LoadKind::Gltf => 0,
                                LoadKind::Image => 1,
                            };
                            r.branch(Some(&mut |e| {
                                e.u32(Some(kind)).str(Some(q.url.as_str()));
                            }));
                        }
                    }));
                })),
            )));
        }
        Cmd::LOAD_UPDATE => {
            let f = fields(pkt)?;
            let url = f.at(0)?.get_str()?;
            let bytes = bytes_at(&f, 1, usize::MAX)?;
            pxl_core::render::gltf::load::gltf_arrived(&url, bytes.as_slice());
            return Ok(Some((0, None)));
        }
        Cmd::LOAD_IMAGE => {
            let f = fields(pkt)?;
            pxl_core::render::gltf::load::image_arrived(&f.at(0)?.get_str()?, u32_at(&f, 1)?);
            return Ok(Some((0, None)));
        }
        Cmd::LOAD_DELETE => {
            pxl_core::render::gltf::load::load_rejected(&fields(pkt)?.at(0)?.get_str()?);
            return Ok(Some((0, None)));
        }
        _ => {}
    }
    let Parts {
        mut world, events, sched, raycast, ..
    } = parts(k, &mut s.events);
    let mut emit = |e: Event| events.push_back(e);
    match cmd {
        Cmd::NOP => {
            let value = match pkt {
                Some(_) => u32_at(&fields(pkt)?, 0)?,
                None => 0,
            };
            return Ok(Some((value as i32, None)));
        }
        Cmd::CHUNK_CREATE => {
            let f = fields(pkt)?;
            sched.add(tag, u64_at(&f, 0)?, i32_at(&f, 3)? as f64, i32_at(&f, 4)? as f64, u32_at(&f, 1)?, u32_at(&f, 2)?, u32_at(&f, 5)?);
        }
        Cmd::CHUNK_DELETE => sched.remove(&mut world, tag, &mut emit),
        Cmd::CHUNK_UPDATE => {
            let f = fields(pkt)?;
            let page = match branch_at(&f, 0)? {
                None => None,
                Some(p) => Some(PageLanes {
                    ground: [u32_at(&p, 0)? as usize, u32_at(&p, 1)? as usize, u32_at(&p, 2)? as usize, u32_at(&p, 3)? as usize],
                    ele: u32_at(&p, 4)? as usize,
                }),
            };
            sched.source(&mut world, tag, page, bool_at(&f, 1)?, &mut emit);
        }
        Cmd::CHUNK_WANT => {
            let f = fields(pkt)?;
            sched.builder.want(&mut world, tag, [u32_at(&f, 0)?, u32_at(&f, 1)?]);
        }
        Cmd::PAGE_CREATE | Cmd::PAGE_DELETE => sched.page(u64_at(&fields(pkt)?, 0)?, cmd == Cmd::PAGE_CREATE),
        Cmd::EDIT_UPDATE => {
            let f = fields(pkt)?;
            let hide = match branch_at(&f, 0)? {
                None => None,
                Some(h) => Some((u64_at(&h, 0)?, i32_at(&h, 1)?, u32_at(&h, 2)?)),
            };
            let carve = match branch_at(&f, 1)? {
                None => None,
                Some(c) => Some((
                    u64_at(&c, 0)?,
                    Carve {
                        lx: number_at(&c, 1)?,
                        ly: number_at(&c, 2)?,
                        lz: number_at(&c, 3)?,
                        size: number_at(&c, 4)?,
                    },
                )),
            };
            sched.builder.set_edits(hide, carve);
        }
        Cmd::RAY_READ => {
            let r = ray_at(&fields(pkt)?)?;
            let Some(h) = raycast.hit(&mut world, sched, [r[0], r[1]], [r[2], r[3], r[4]], [r[5], r[6], r[7]], r[8]) else {
                return Ok(Some((0, None)));
            };
            let row = [
                h.material_class as f64,
                h.normal[0],
                h.normal[1],
                h.normal[2],
                h.point[0],
                h.point[1],
                h.point[2],
                h.t,
                h.box_min[0],
                h.box_min[1],
                h.box_min[2],
                h.box_max[0],
                h.box_max[1],
                h.box_max[2],
            ];
            return Ok(Some((
                1,
                Some(packet(&mut |b| {
                    b.f64_array(Some(&F64Array::from(&row)));
                })),
            )));
        }
        Cmd::DECOR_READ => {
            let r = ray_at(&fields(pkt)?)?;
            let Some(h) = pxl_core::world::raycast::hit_decor(&world, sched, [r[0], r[1]], [r[2], r[3], r[4]], [r[5], r[6], r[7]], r[8]) else {
                return Ok(Some((0, None)));
            };
            return Ok(Some((
                1,
                Some(packet(&mut |b| {
                    b.u32(Some(h.chunk)).u32(Some(h.band)).u32(Some(h.pos)).f64(Some(h.t));
                })),
            )));
        }
        Cmd::OBJECT_UPDATE
        | Cmd::OBJECT_DELETE
        | Cmd::OBJECT_REINDEX
        | Cmd::STATE_UPDATE
        | Cmd::TERRAIN_UPDATE
        | Cmd::DECOR_KIND_UPDATE
        | Cmd::DECOR_KIND_MODEL
        | Cmd::OBJECT_READ
        | Cmd::OBJECT_LANES
        | Cmd::OBJECT_CELL
        | Cmd::OBJECT_COLUMNS
        | Cmd::OBJECT_STAT
        | Cmd::MODEL_SEAL
        | Cmd::CHANGE_UPDATE
        | Cmd::CHANGE_READ
        | Cmd::CHANGE_DELETE
        | Cmd::FRAME_READ
        | Cmd::CHUNK_READ
        | Cmd::ENV_LANES
        | Cmd::CHANGE_LANES
        | Cmd::DRAW_READ
        | Cmd::SHADER_READ
        | Cmd::SHADER_PIPELINE
        | Cmd::POOL_CREATE
        | Cmd::POOL_READ
        | Cmd::POOL_ACQUIRE
        | Cmd::POOL_RELEASE
        | Cmd::POOL_FREE
        | Cmd::VOLUME_POOLS
        | Cmd::VOLUME_STAT
        | Cmd::VOLUME_CREATE
        | Cmd::VOLUME_READ
        | Cmd::VOLUME_DELETE
        | Cmd::VOLUME_FILL
        | Cmd::VOLUME_VOXEL
        | Cmd::VOLUME_SUB_VOXEL
        | Cmd::VOLUME_SEAL
        | Cmd::BAKE_CREATE
        | Cmd::BAKE_STEP
        | Cmd::BAKE_MESH
        | Cmd::BAKE_FINISH
        | Cmd::BAKE_DELETE
        | Cmd::SCAN_CREATE
        | Cmd::SCAN_STEP
        | Cmd::SCAN_SET
        | Cmd::SCAN_DELETE
        | Cmd::TERRAIN_READ
        | Cmd::TERRAIN_PAINT
        | Cmd::OBJECT_PAINT
        | Cmd::RAY_COLUMN => {
            unreachable!("applied before the parts are taken")
        }
        Cmd::GHOST_BOX => {
            let f = fields(pkt)?;
            let at = [number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?];
            sched.ghost.set_box(&mut world, &mut sched.draw, at, number_at(&f, 3)?, u32_at(&f, 4)?, u32_at(&f, 5)?, bool_at(&f, 6)?);
        }
        Cmd::GHOST_DECOR => {
            let f = fields(pkt)?;
            let at = [number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?];
            sched.ghost.set_decor(&mut world, &mut sched.draw, at, u32_at(&f, 3)?, number_at(&f, 4)? as f32);
        }
        Cmd::GHOST_DELETE => sched.ghost.clear(&mut world, &mut sched.draw),
        Cmd::PUCK_UPDATE => {
            let f = fields(pkt)?;
            let zone = floats_at(&f, 3, 7)?;
            let at = [number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?];
            // SAFETY: the zone's floats, in the arena, until this message is advanced past.
            unsafe {
                sched.markers.set_zone(
                    &mut world,
                    &mut sched.draw,
                    at,
                    zone.length() as u32 / 7,
                    zone.as_slice().as_ptr() as usize,
                    number_at(&f, 4)?,
                    number_at(&f, 5)?,
                )
            };
        }
        Cmd::PUCK_BEACON => {
            let f = fields(pkt)?;
            sched.markers.set_beacon([number_at(&f, 0)?, number_at(&f, 1)?], bool_at(&f, 2)?);
        }
        Cmd::PUCK_DELETE => sched.markers.clear(&mut world, &mut sched.draw),
        Cmd::BILLBOARD_CREATE => {
            let order = u32_at(&fields(pkt)?, 0)?;
            return Ok(Some((sched.billboards.new_set(order) as i32, None)));
        }
        Cmd::BILLBOARD_UPDATE => {
            let lane = floats_at(&fields(pkt)?, 0, 10)?;
            // SAFETY: the instances' floats, in the arena, until this message is advanced past.
            unsafe { sched.billboards.write(&mut world, &mut sched.draw, tag, lane.length() as u32 / 10, lane.as_slice().as_ptr() as usize) };
        }
        Cmd::BILLBOARD_PLACE => {
            let f = fields(pkt)?;
            sched.billboards.place(tag, [number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?]);
        }
        Cmd::BILLBOARD_FOLLOW => {
            let f = fields(pkt)?;
            sched.billboards.follow(tag, u32_at(&f, 0)?, number_at(&f, 1)?);
        }
        Cmd::BILLBOARD_DELETE => sched.billboards.remove(&mut world, &mut sched.draw, tag),
        Cmd::ACTOR_CREATE => {
            let model = u32_at(&fields(pkt)?, 0)?;
            return Ok(Some((sched.models.new_actor(&mut world, &mut sched.draw, model).map_or(-1, |a| a as i32), None)));
        }
        Cmd::ACTOR_UPDATE => {
            let f = fields(pkt)?;
            let (at, rotation_z, visible) = ([number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?], number_at(&f, 3)?, bool_at(&f, 4)?);
            if let Some(a) = sched.models.actor(tag) {
                a.at = at;
                a.rotation_z = rotation_z as f32;
                a.visible = visible;
            }
        }
        Cmd::ACTOR_PLAY => {
            let f = fields(pkt)?;
            sched.models.play(tag, u32_at(&f, 0)?, number_at(&f, 1)? as f32);
        }
        Cmd::ACTOR_DELETE => sched.models.remove_actor(&mut world, &mut sched.draw, tag),
        Cmd::DEBUG_UPDATE => {
            let f = fields(pkt)?;
            let vertices = floats_at(&f, 3, 6)?;
            let at = [number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?];
            // SAFETY: the vertices' floats, in the arena, until this message is advanced past.
            unsafe { sched.debug.set(&mut world, &mut sched.draw, at, vertices.length() as u32 / 6, vertices.as_slice().as_ptr() as usize) };
        }
        Cmd::ENV_UPDATE => {
            let lane = floats_at(&fields(pkt)?, 0, pxl_core::world::environment::THEME_FLOATS)?;
            let theme = <[f32; pxl_core::world::environment::THEME_FLOATS]>::try_from(lane.as_slice())
                .map_err(|_| MarshalError::new(Errno::EBADMSG, Some(&format!("a theme of {} floats", lane.length())), None))?;
            sched.environment.set_theme(&theme);
        }
        Cmd::ENV_OBSERVE => {
            let f = fields(pkt)?;
            sched.environment.observe(number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?);
        }
        Cmd::MODEL_CREATE => {
            let f = fields(pkt)?;
            let Some((id, at)) = sched.models.new_model(&mut world, u32_at(&f, 0)?, u32_at(&f, 1)?, u32_at(&f, 2)?) else {
                return Ok(Some((-1, None)));
            };
            return Ok(Some((
                id as i32,
                Some(packet(&mut |b| {
                    b.u32(Some(at as u32));
                })),
            )));
        }
        Cmd::MODEL_READ => {
            let model = u32_at(&fields(pkt)?, 0)?;
            return Ok(Some((sched.models.mesh(model).map_or(0, |m| m.handle) as i32, None)));
        }
        Cmd::MODEL_JOINT => {
            let f = fields(pkt)?;
            let pose = floats_at(&f, 2, 42)?;
            let p = pose.as_slice();
            if p.len() != 42 {
                return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("a joint of {} floats, not 42", p.len())), None));
            }
            let (mut rest, mut pre, mut ib) = ([0.0; 10], [0.0; 16], [0.0; 16]);
            rest.copy_from_slice(&p[0..10]);
            pre.copy_from_slice(&p[10..26]);
            ib.copy_from_slice(&p[26..42]);
            sched.models.joint(tag, u32_at(&f, 0)?, i32_at(&f, 1)?, rest, pre, ib);
        }
        Cmd::MODEL_CLIP => {
            // A query's tag is its call's, so the model is in the packet.
            let f = fields(pkt)?;
            return Ok(Some((sched.models.clip(u32_at(&f, 0)?, number_at(&f, 1)? as f32) as i32, None)));
        }
        Cmd::MODEL_CHANNEL => {
            let f = fields(pkt)?;
            let path = u32_at(&f, 2)?;
            let (times, values) = (floats_at(&f, 4, 1)?, floats_at(&f, 5, if path == 1 { 4 } else { 3 })?);
            let width = if path == 1 { 4 } else { 3 };
            if values.length() != times.length() * width {
                return Err(MarshalError::new(
                    Errno::EBADMSG,
                    Some(&format!("{} keys and {} values of width {width}", times.length(), values.length())),
                    None,
                ));
            }
            // SAFETY: the keys' floats, in the arena, until this message is
            // advanced past; the channel copies them.
            unsafe {
                sched.models.channel(
                    core::ptr::null(),
                    tag,
                    u32_at(&f, 0)?,
                    u32_at(&f, 1)?,
                    path as u8,
                    bool_at(&f, 3)?,
                    times.length() as u32,
                    times.as_slice().as_ptr() as usize,
                    values.as_slice().as_ptr() as usize,
                )
            };
        }
        Cmd::CHUNK_BUILD => sched.build_now(&mut world, tag, &now, &mut emit),
        Cmd::CHUNK_TICK => {
            let f = fields(pkt)?;
            let order = f.at(0)?.get_array::<u32>()?;
            let budget_ms = number_at(&f, 1)?;
            sched.builder.tick(&mut world, order.as_slice(), budget_ms, &now, &mut |e| emit(Event::Build(e)));
        }
        Cmd::CHUNK_DECOR => {
            let show = bool_at(&fields(pkt)?, 0)?;
            sched.decor_now(&mut world, tag, show, &mut emit);
        }
        Cmd::ENV_READ => {
            let f = fields(pkt)?;
            sched.environment.daylight(number_at(&f, 0)?, number_at(&f, 1)?, number_at(&f, 2)?);
            let d = sched.environment.daylight;
            return Ok(Some((
                0,
                Some(packet(&mut |b| {
                    b.f64(Some(d[0])).f64(Some(d[1]));
                })),
            )));
        }
        // Signals: the kernel raises these, and a shell submitting one is
        // asking for something no handler does.
        // Applied before the world is borrowed, above.
        Cmd::WORLD_CREATE | Cmd::WORLD_LANES | Cmd::WORLD_ACTION | Cmd::WORLD_SHEET | Cmd::WORLD_UPDATE | Cmd::WORLD_READ | Cmd::INPUT_UPDATE | Cmd::INPUT_POINTER | Cmd::INPUT_WHEEL | Cmd::SENSOR_UPDATE | Cmd::SENSOR_ERROR | Cmd::SENSOR_ORIENTATION | Cmd::SENSOR_PERMISSION | Cmd::SENSOR_VISIBILITY | Cmd::PAGE_TERRAIN | Cmd::PAGE_POI | Cmd::FETCH_READ | Cmd::FETCH_UPDATE | Cmd::FETCH_GEOCODE | Cmd::FETCH_SEARCH | Cmd::LOAD_READ | Cmd::LOAD_UPDATE | Cmd::LOAD_IMAGE | Cmd::LOAD_DELETE => return Ok(None),
        Cmd::CHUNK_VISIBLE | Cmd::CHUNK_LAYOUT | Cmd::CHUNK_BLEND | Cmd::TIER_CREATE | Cmd::TIER_DELETE | Cmd::RUNS_UPDATE | Cmd::DECOR_CREATE => return Ok(None),
    }
    Ok(Some((0, None)))
}

/// Applies every submission waiting. What they make goes into the completion
/// ring as far as it has room; the rest waits in the module, in order, for
/// the next call — nothing is dropped, and nothing sent waits to be applied,
/// which is what lets a read see every write before it. Replies to what asks
/// (`ACKREQ`), and to what failed — a command nobody numbers, a packet that
/// does not read — since a command's effect is otherwise its signals.
fn drain(s: &mut State) -> u32 {
    let mut posted = flush(s);
    loop {
        let at = s.sq.peek();
        if at < 0 {
            break;
        }
        let at = at as u32;
        let (tag, number, flags) = (s.sq.tag(at), s.sq.cmd(at), s.sq.flags(at));
        let (result, reply) = match s.sq.packet(at) {
            Ok(pkt) => match apply(s, tag, number, &pkt) {
                Ok(Some((value, reply))) => (Ok(value), reply),
                Ok(None) => (Err(Errno::ENOSYS as i32), None),
                Err(e) => (Err(e.errno as i32), None),
            },
            Err(e) => (Err(e.errno as i32), None),
        };
        s.sq.advance();
        if flags & FrameFlag::ACKREQ as u32 != 0 || result.is_err() {
            // Queued rather than pushed: a reply's packet may not fit the
            // arena this turn, and a reply is never dropped.
            s.replies.push_back((tag, number, result.unwrap_or_else(|errno| errno), reply));
        }
        posted += flush(s);
    }
    // The rows the glue reads by address (the build stat, the census) are
    // as of the last command, as they are of the last tick.
    after();
    posted
}

/// The state split into what a build, a tick or a raycast borrows at once:
/// the world the core reads and writes, and the parts that drive it.
struct Parts<'a> {
    world: World<'a>,
    events: &'a mut std::collections::VecDeque<Event>,
    sched: &'a mut Scheduler,
    raycast: &'a mut Raycast,
}

fn parts<'a>(k: &'a mut Kernel, events: &'a mut std::collections::VecDeque<Event>) -> Parts<'a> {
    Parts {
        world: World {
            volumes: opened(&mut k.volumes),
            pools: &mut k.pools,
            materials: &k.terrain,
            store: &k.objects,
            painter: &mut k.painter,
        },
        events,
        sched: &mut k.sched,
        raycast: &mut k.raycast,
    }
}

fn after() {
    let k = kernel();
    let st = k.sched.builder.stat;
    let c = k.sched.builder.bake_context().counters;
    k.build_stat = [
        st.direct,
        st.scans,
        st.bakes,
        st.starved,
        c.total_runs,
        c.culled,
        c.faces_possible,
        c.faces_kept,
        c.coarse_total,
        c.coarse_low_fill,
        ALLOC.used() as u32,
    ];
    let c = k.sched.census;
    k.census = [
        c.chunks as f64,
        c.in_frustum as f64,
        c.settling as f64,
        c.blend_active as f64,
        c.pending as f64,
        c.offscreen_pending as f64,
        c.oldest_visible_ms,
        c.oldest_release_ms,
        c.committed as f64,
        c.released as f64,
        c.retained_on,
        c.retained_off,
        c.pressure as f64,
    ];
}

/// The frame's turn (Interrupt::TICK): applies the submissions waiting, then
/// the scheduler's pass over every chunk with the frame the glue wrote at
/// FRAME_READ's address. 1 when the canvas has something new to draw.
fn tick(s: &mut State) -> u32 {
    drain(s);
    let k = kernel();
    // Events the ring had no room for are posted first; a tick on top of
    // them would only queue more. The glue reads the ring dry and drains
    // again before the next tick, so this is a frame lost only when it
    // does not.
    if !s.events.is_empty() || k.volumes.is_none() {
        return 1;
    }
    let f = k.frame;
    let mut frame = Frame::default();
    frame.root.copy_from_slice(&f[0..16]);
    frame.origin.copy_from_slice(&f[16..19]);
    frame.planes.copy_from_slice(&f[19..43]);
    frame.camera.copy_from_slice(&f[43..46]);
    frame.lead.copy_from_slice(&f[46..49]);
    frame.viewport_h = f[49];
    frame.fov_deg = f[50];
    frame.delta_s = f[51];
    frame.budget_ms = f[52];
    frame.hard_slack_ms = f[53];
    frame.pixel_dither = f[54] != 0.0;
    frame.retain_budget_bytes = f[55];
    frame.anchor = (f[58] != 0.0).then_some([f[56], f[57]]);
    frame.render_dist = f[59];
    frame.range_hysteresis = f[60];
    frame.show_decor = f[61] != 0.0;
    let Parts { mut world, events, sched, .. } = parts(k, &mut s.events);
    let draw = sched.tick(&mut world, frame, &now, &mut |e| events.push_back(e));
    flush(s);
    after();
    draw as u32
}

// ---- The billboards (crates/pxl/core/src/world/billboard.rs) ----

// ---- The shaders and the pipelines (crates/pxl/core/src/render/shader.rs) ----

// ---- The dev gizmos (crates/pxl/core/src/world/debug.rs) ----

// ---- The environment (crates/pxl/core/src/world/environment.rs) ----

// ---- The location puck (crates/pxl/core/src/world/marker.rs) ----

// ---- Models and actors (crates/pxl/core/src/world/model.rs) ----

// ---- The host's hooks (pxl_core::host, pxl_core::global) ----

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

fn hook_local_time() -> pxl_core::host::LocalTime {
    let mut t = [0.0f64; 5];
    // SAFETY: the host writes five doubles into `t`.
    unsafe { host_local_time(t.as_mut_ptr()) };
    pxl_core::host::LocalTime {
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

fn hook_entropy() -> u64 {
    // SAFETY: an import with no arguments.
    unsafe { ((host_entropy() as u64) << 32) | host_entropy() as u64 }
}

/// Hands the kernel the host's clock, storage, local time, text measure and
/// entropy, before anything reads them.
fn install_host() {
    pxl_core::global::set_clock(hook_now);
    pxl_core::host::set_storage(pxl_core::host::Storage {
        get: hook_storage_get,
        set: hook_storage_set,
    });
    pxl_core::host::set_local_time(hook_local_time);
    pxl_core::host::set_measure_text(hook_measure_text);
    xpute_core::kit::rng::set_entropy(hook_entropy);
}
