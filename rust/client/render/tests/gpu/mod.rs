//! The tests that run a pass written by hand share this device and its helpers. A pass by
//! hand is a WGSL shader that carries out what pxl-engine's Rust does, and the tests compare the
//! two. A module comes as build.rs assembles it for the renderer, or from the file a mutant sweep
//! names. A wait on the device fails the test after `WAIT`, because a shader may loop forever,
//! and a dispatch the device stops short fails the test too. The runs of the cast here dispatch
//! their passes in the order the renderer does (ray/cast.rs `walk_run`).

#![allow(dead_code, reason = "each test reads its own of it")]

mod lock;

use std::collections::BTreeMap;
use std::time::Duration;

use pxl_engine::answer::Kind;
use pxl_engine::light::{self, Light, StartGrid};
use pxl_engine::{cast, count, node_cache, ray};
use pxl_render::frame::shader::record::{
    compute, src, COMPUTE_QUEUE, COMPUTE_QUEUE_ASK, COMPUTE_QUEUE_ASK_PARKED,
    COMPUTE_QUEUE_CORNERS, COMPUTE_QUEUE_CORNERS_PARKED, COMPUTE_QUEUE_LAST,
    COMPUTE_QUEUE_LAST_CORNERS, COMPUTE_QUEUE_LAST_LIGHT, COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
    COMPUTE_QUEUE_LAST_LIGHT_SAMPLES, COMPUTE_QUEUE_LAST_SAMPLES, COMPUTE_QUEUE_LIGHT,
    COMPUTE_QUEUE_LIGHT_CORNERS, COMPUTE_QUEUE_LIGHT_CORNERS_PARKED, COMPUTE_QUEUE_LIGHT_PARKED,
    COMPUTE_QUEUE_LIGHT_SAMPLES, COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED, COMPUTE_QUEUE_PARKED,
    COMPUTE_QUEUE_SAMPLES, COMPUTE_QUEUE_SAMPLES_PARKED,
};
use pxl_spec::space::coord::q3::FRAC_W;

/// How long a test waits for the device to finish its work before the test fails.
const WAIT: Duration = Duration::from_secs(60);
/// How many frames a run may take to ask the world for everything its rays need (`Gpu::run`).
const FRAMES_TO_ASK: u32 = 64;

/// Whether a mutant sweep runs this test. In a sweep, the file `MUT_WGSL` names stands in for
/// the module that holds the entry, and the test stops at its first case that differs.
pub fn mutant() -> bool {
    std::env::var_os("MUT_WGSL").is_some()
}

/// The source of the module that holds `entry`. It is the file `MUT_WGSL` names where that
/// file holds the entry, and `assembled` otherwise.
pub fn source(assembled: &str, entry: &str) -> String {
    if let Some(path) = std::env::var_os("MUT_WGSL") {
        let text = std::fs::read_to_string(&path).expect("the mutant's module");
        if text.contains(&format!("fn {entry}(")) {
            return text;
        }
    }
    assembled.to_string()
}

pub fn bytes(words: &[u32]) -> &[u8] {
    // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) }
}

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Whether the next run starts with its frame's asks spent, so a ray goes only as deep as
    /// the node cache holds.
    pub asks_spent: std::cell::Cell<bool>,
}

impl Gpu {
    /// A device with the adapter's own limits, or `None` where no adapter answers. The process
    /// holds the GPU from here on (`lock::hold`).
    pub fn new() -> Option<Gpu> {
        lock::hold();
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .unwrap();
        Some(Gpu {
            device,
            queue,
            asks_spent: std::cell::Cell::new(false),
        })
    }

    /// Waits until every submission is done, and fails the test past `WAIT`.
    pub fn wait(&self) {
        wait(&self.device);
    }

    pub fn buf(&self, words: &[u32], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        use wgpu::util::DeviceExt as _;
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytes(words),
                usage: usage | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            })
    }

    pub fn read(&self, from: &wgpu::Buffer) -> Vec<u32> {
        read_buf(&self.device, &self.queue, from)
    }

    /// The pipeline of `entry` in `assembled`, or in the mutant's module (`source`). Its group's
    /// layout is the one the module's bindings imply. A module the device refuses fails the test.
    pub fn pipeline(&self, assembled: &str, entry: &str) -> wgpu::ComputePipeline {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(entry),
                source: wgpu::ShaderSource::Wgsl(source(assembled, entry).into()),
            });
        let p = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            });
        if let Some(e) = pollster::block_on(scope.pop()) {
            panic!("{entry} refused: {e}");
        }
        p
    }

    /// Dispatches `groups` workgroups of `p` over `binds` and waits for them. A command buffer
    /// the device stopped short fails the test.
    pub fn dispatch(
        &self,
        p: &wgpu::ComputePipeline,
        binds: &[(u32, wgpu::BindingResource)],
        groups: [u32; 3],
    ) {
        let entries: Vec<_> = binds
            .iter()
            .map(|(b, resource)| wgpu::BindGroupEntry {
                binding: *b,
                resource: resource.clone(),
            })
            .collect();
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &p.get_bind_group_layout(0),
            entries: &entries,
        });
        // The same command buffer clears this word after the pass. When Metal aborts a command
        // buffer midway, it runs nothing after the abort, yet wgpu counts the buffer done. A word
        // left set shows the abort.
        let ran = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        ran.slice(..)
            .get_mapped_range_mut()
            .unwrap()
            .copy_from_slice(&[0xff; 4]);
        ran.unmap();
        let mut e = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            pass.set_pipeline(p);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(groups[0], groups[1], groups[2]);
        }
        e.clear_buffer(&ran, 0, None);
        self.queue.submit([e.finish()]);
        ran.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        self.wait();
        let cleared = ran
            .slice(..)
            .get_mapped_range()
            .unwrap()
            .iter()
            .all(|&b| b == 0);
        assert!(
            cleared,
            "a dispatch of {groups:?} workgroups: the command buffer did not run to its end"
        );
    }

    /// Runs the light's beam rays as the cast runs them (corner.wgsl over beam.light.wgsl),
    /// over a map `size` texels a side. It first lists every beam ray of the whole map in `list`
    /// (light_list.wgsl `light_item_at`), then runs them through `corners`. `world` binds what
    /// the passes read, with the list at 8, `node_cache` at 9 and the light's header
    /// at 11.
    pub fn light_beam(
        &self,
        world: &[(u32, wgpu::BindingResource)],
        list: &wgpu::Buffer,
        node_cache: &wgpu::Buffer,
        size: u32,
    ) {
        let mut words = self.read(list);
        list_every(&mut words, size, light::LIST_CORNERS, 1);
        self.queue.write_buffer(list, 0, bytes(&words));
        self.corners(true, world, node_cache);
    }

    /// Runs an observer's holes as the cast runs them (ray/cast.rs `walk_run`), the light's
    /// where `light` is set and the eye's otherwise. Two walks read the node cache alone, and
    /// an ask follows each. The queue's hits are then copied into `node_cache`
    /// (`node_cache::HITS_ARGS`), as the cast copies them. The last walk runs after. `all`
    /// binds what any of the passes reads except the queue's words: the eye at 0, the world at
    /// 1, 2 and 4 to 7, the rays or the map at 3, the observer's list at 8, the node cache at 9
    /// and the light's header at 11. Each pass takes the bindings its entry reads.
    pub fn holes(
        &self,
        light: bool,
        all: &[(u32, wgpu::BindingResource)],
        node_cache: &wgpu::Buffer,
    ) {
        let (queue, ids) = if light {
            (
                ray::QUEUE_LIGHT_HOLES,
                [
                    COMPUTE_QUEUE_LIGHT,
                    COMPUTE_QUEUE_ASK,
                    COMPUTE_QUEUE_LIGHT_PARKED,
                    COMPUTE_QUEUE_ASK_PARKED,
                    COMPUTE_QUEUE_LAST_LIGHT,
                ],
            )
        } else {
            (
                ray::QUEUE_EYE_HOLES,
                [
                    COMPUTE_QUEUE,
                    COMPUTE_QUEUE_ASK,
                    COMPUTE_QUEUE_PARKED,
                    COMPUTE_QUEUE_ASK_PARKED,
                    COMPUTE_QUEUE_LAST,
                ],
            )
        };
        // The light's walks read its header at 11. The eye's walks and the asks do not.
        let hdr: &[u32] = if light { &[11] } else { &[] };
        let walk = [&[0, 3, 8, 9, 10][..], hdr].concat();
        let last = [&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9][..], hdr].concat();
        let keep = [&walk[..], &[0, 1, 2, 4, 5, 6, 7, 9, 10], &last[..]];
        self.run(queue, ids, keep, all, node_cache, None);
    }

    /// Runs an observer's beam rays as the cast runs them (ray/cast.rs `walk_run`), the
    /// light's where `light` is set and the eye's otherwise. The passes and bindings follow
    /// `samples`.
    pub fn corners(
        &self,
        light: bool,
        all: &[(u32, wgpu::BindingResource)],
        node_cache: &wgpu::Buffer,
    ) {
        let ids = if light {
            [
                COMPUTE_QUEUE_LIGHT_CORNERS,
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
                COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
            ]
        } else {
            [
                COMPUTE_QUEUE_CORNERS,
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
                COMPUTE_QUEUE_LAST_CORNERS,
            ]
        };
        let queue = if light {
            ray::QUEUE_LIGHT_CORNERS
        } else {
            ray::QUEUE_EYE_CORNERS
        };
        self.lattice(queue, ids, light, all, node_cache, None);
    }

    /// Runs an observer's samples as the cast runs them (ray/cast.rs `walk_run`), the
    /// light's where `light` is set and the eye's otherwise. Two walks read the node cache
    /// alone, an ask follows each, and the last walk runs after. `all` binds what any of the
    /// passes reads except the queue's words: the eye at 0, the world at 1, 2 and 4 to 7, the
    /// observer's list at 8, the node cache at 9 and the light's header at 11. Each pass takes
    /// the bindings its entry reads.
    pub fn samples(
        &self,
        light: bool,
        all: &[(u32, wgpu::BindingResource)],
        node_cache: &wgpu::Buffer,
    ) {
        self.samples_last_cache(light, all, node_cache, None);
    }

    /// `samples`, with the last walk, which asks the world, over the node cache `last_cache` in place
    /// of `node_cache` where `last_cache` is given. The walks before it and their asks read and write
    /// `node_cache`.
    pub fn samples_last_cache(
        &self,
        light: bool,
        all: &[(u32, wgpu::BindingResource)],
        node_cache: &wgpu::Buffer,
        last_cache: Option<&wgpu::Buffer>,
    ) {
        let ids = if light {
            [
                COMPUTE_QUEUE_LIGHT_SAMPLES,
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
                COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
            ]
        } else {
            [
                COMPUTE_QUEUE_SAMPLES,
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
                COMPUTE_QUEUE_LAST_SAMPLES,
            ]
        };
        let queue = if light {
            ray::QUEUE_LIGHT_SAMPLES
        } else {
            ray::QUEUE_EYE_SAMPLES
        };
        self.lattice(queue, ids, light, all, node_cache, last_cache);
    }

    /// Runs the passes `ids` of a lattice, the light's where `light` is set and the eye's
    /// otherwise, over `node_cache` and `last_cache` as `samples_last_cache` says.
    fn lattice(
        &self,
        queue: u32,
        ids: [u32; 5],
        light: bool,
        all: &[(u32, wgpu::BindingResource)],
        node_cache: &wgpu::Buffer,
        last_cache: Option<&wgpu::Buffer>,
    ) {
        // The light's walks read its header at 11. The eye's walks and every lattice's asks do
        // not.
        let hdr: &[u32] = if light { &[11] } else { &[] };
        let walk = [&[0, 8, 9, 10][..], hdr].concat();
        let last = [&[0, 1, 2, 4, 5, 6, 7, 8, 9][..], hdr].concat();
        let keep = [&walk[..], &[0, 1, 2, 4, 5, 6, 7, 9, 10], &last[..]];
        self.run(queue, ids, keep, all, node_cache, last_cache);
    }

    /// Runs the passes `ids` over `all` and the view of queue `queue` the cast binds: the walk, its asks, the walk of
    /// the parked rays, their asks, and the last walk. `keep` names the bindings that the walks,
    /// the asks and the last walk each take. The last walk reads the node cache `last_cache` where it
    /// is given. In the holes' runs the hits are copied as `holes` says. A run that asks past `ray::ASKS_A_FRAME` ends rays as lacks. It
    /// then runs again over the node cache it left, as the frames after it would, until a run
    /// asks no more than a frame's asks. The words that run leaves are what a test reads.
    fn run(
        &self,
        queue_no: u32,
        [walk_id, ask_id, walk_parked_id, ask_parked_id, last_id]: [u32; 5],
        [walk_keep, ask_keep, last_keep]: [&[u32]; 3],
        all: &[(u32, wgpu::BindingResource)],
        node_cache: &wgpu::Buffer,
        last_cache: Option<&wgpu::Buffer>,
    ) {
        let hits = matches!(queue_no, ray::QUEUE_EYE_HOLES | ray::QUEUE_LIGHT_HOLES);
        let queue = self.buf(
            &vec![0u32; ray::QUEUE_WORDS as usize],
            wgpu::BufferUsages::STORAGE,
        );
        let head = ray::queue_at(queue_no);
        let pipeline = |id| {
            let (shader, entry, _) = compute(id);
            self.pipeline(src(shader), entry)
        };
        let take = |keep: &[u32]| {
            let mut binds: Vec<_> = all
                .iter()
                .filter(|(b, _)| keep.contains(b))
                .cloned()
                .collect();
            if keep.contains(&10) {
                binds.push((
                    10,
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &queue,
                        offset: u64::from(head) * 4,
                        size: wgpu::BufferSize::new(u64::from(ray::QUEUE_WORDS - head) * 4),
                    }),
                ));
            }
            binds
        };
        let [walk, ask, walk_parked, ask_parked, last] =
            [walk_id, ask_id, walk_parked_id, ask_parked_id, last_id].map(pipeline);
        let walk_binds = take(walk_keep);
        let ask_binds = take(ask_keep);
        let mut last_binds = take(last_keep);
        if let Some(last_cache) = last_cache {
            for (b, resource) in &mut last_binds {
                if *b == 9 {
                    *resource = last_cache.as_entire_binding();
                }
            }
        }
        let groups = [ray::QUEUE_GROUP_CNT, 1, 1];
        let word = |w: u32| u64::from(w) * 4;
        // The asks are counted in their observer's lane, which the asks' module reads off
        // the queue it is bound to (ask_run.wgsl). The other observer's stays as it was.
        let other_asked = (if queue_no >= ray::QUEUE_LIGHT_CORNERS {
            node_cache::lane_at(count::EYE, count::ASK)
        } else {
            node_cache::lane_at(count::LIGHT, count::ASK)
        }) as usize;
        // Both observers count, as a frame that casts for the eye and the light does.
        self.queue
            .write_buffer(node_cache, word(node_cache::CNT_ON), bytes(&[0b11u32]));
        let before = self.read(node_cache)[other_asked];
        for _ in 0..FRAMES_TO_ASK {
            // Each run is a frame of its own. The count of asks and the last walk's next ray
            // start at zero, as the host and the asks set them each frame the cast runs.
            let mut none = [0u32; (node_cache::ASKED + 1 - node_cache::LAST_NEXT) as usize];
            if self.asks_spent.get() {
                none[(node_cache::ASKED - node_cache::LAST_NEXT) as usize] = ray::ASKS_A_FRAME;
            }
            for t in [Some(node_cache), last_cache].into_iter().flatten() {
                self.queue
                    .write_buffer(t, word(node_cache::LAST_NEXT), bytes(&none));
            }
            self.queue
                .write_buffer(&queue, word(head), bytes(&ray::QUEUE_EMPTY));
            self.dispatch(&walk, &walk_binds, groups);
            self.dispatch(&ask, &ask_binds, groups);
            self.dispatch(&walk_parked, &walk_binds, groups);
            self.dispatch(&ask_parked, &ask_binds, groups);
            if hits {
                let q = self.read(&queue);
                let h = (head + ray::QUEUE_HITS) as usize;
                self.queue.write_buffer(
                    node_cache,
                    word(node_cache::HITS_ARGS),
                    bytes(&q[h..h + 4]),
                );
            }
            if let Some(last_cache) = last_cache {
                let cnt = self.read(node_cache)[node_cache::LAST_CNT as usize];
                self.queue
                    .write_buffer(last_cache, word(node_cache::LAST_CNT), bytes(&[cnt]));
            }
            self.dispatch(&last, &last_binds, groups);
            let words = self.read(last_cache.unwrap_or(node_cache));
            assert_eq!(
                self.read(node_cache)[other_asked],
                before,
                "the asks of queue {queue_no} were counted in the other observer's lane"
            );
            if self.asks_spent.get() || words[node_cache::ASKED as usize] <= ray::ASKS_A_FRAME {
                return;
            }
        }
        panic!("a run still asks past a frame's asks after {FRAMES_TO_ASK} frames");
    }
}

/// Fills list `which` of the light's list `words`, over a map `size` texels a side, with every
/// item of its level, as a cast that damages every tile lays it (light_enlist.wgsl `enlist`).
/// Returns the workgroups a pass dispatches over that list when each entry is `per` items a
/// side (light_list.wgsl `light_item`).
pub fn list_every(words: &mut [u32], size: u32, which: u32, per: u32) -> [u32; 2] {
    let across = if which == light::LIST_CORNERS {
        light::beam_across(size)
    } else {
        light::tiles(size)
    };
    let at = light::list_cnt_at(size, which) as usize;
    words[at] = across * across;
    for k in 0..across * across {
        words[at + 1 + k as usize] = ((k / across) << 16) | (k % across);
    }
    let threads = across * across * per * per;
    [
        light::LIST_ACROSS,
        threads.div_ceil(light::LIST_ACROSS * light::LIST_LANE_CNT),
    ]
}

/// Waits until every submission on `device` is done, and fails the test past `WAIT`.
fn wait(device: &wgpu::Device) {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(WAIT),
        })
        .unwrap_or_else(|e| panic!("the device did not finish in {WAIT:?}: {e:?}"));
}

/// Lays a drawable entry in slot `slot` of the node cache's words `cache` (pxl-engine
/// `node_cache`, `record`). The entry holds the node `n` as a walk from the origin `o` holds it.
/// A walk of frame 1 kept it at the node's own zoom, its answer is `kind` and `asset`, and
/// `state` says what draws it.
pub fn lay_entry(
    cache: &mut [u32],
    slot: u32,
    o: [i32; 3],
    n: &cast::Node,
    (kind, asset): (Kind, [u32; 2]),
    state: u32,
) {
    let e = node_cache::entry_at(slot) as usize;
    let zooms = n.zoom | n.zoom << pxl_engine::ZOOM_BITS;
    cache[node_cache::key_at(slot) as usize] = node_cache::PASS_CNT | zooms << node_cache::TAG_BITS;
    cache[e + node_cache::W_KIND as usize] = kind as u32;
    cache[e + node_cache::W_ASSET as usize] = asset[0];
    cache[e + node_cache::W_ASSET as usize + 1] = asset[1];
    let w = node_cache::W_NODE as usize;
    cache[e + w] = n.zoom;
    let idx = n.idx(o);
    for a in 0..3 {
        cache[e + w + 1 + a] = idx[a] as u32;
    }
    cache[node_cache::state_at(slot) as usize] = state;
}

/// Lays the node `n`, as a walk from the origin `o` holds it, in the first free slot of its
/// bucket in the node cache's words `cache`, as a walk that asked it keeps it (ask.wgsl `keep`).
/// Its answer is `answer`, a kind and an asset, and nothing draws it yet. Returns the slot, or
/// `None` where the node is kept already or its bucket is full.
pub fn lay_kept(
    cache: &mut [u32],
    o: [i32; 3],
    n: &cast::Node,
    answer: (Kind, [u32; 2]),
) -> Option<u32> {
    let idx = n.idx(o);
    let first = node_cache::bucket_of(n.zoom, idx);
    let mut free = None;
    for slot in first..first + node_cache::BUCKET {
        if holds_node(cache, slot) {
            let w = (node_cache::entry_at(slot) + node_cache::W_NODE) as usize;
            if cache[w] == n.zoom && [1, 2, 3].map(|a| cache[w + a] as i32) == idx {
                return None;
            }
        } else if free.is_none() {
            free = Some(slot);
        }
    }
    let slot = free?;
    lay_entry(cache, slot, o, n, answer, 0);
    Some(slot)
}

/// The node of the entry at word `e` of the node cache's words `cache`, as a walk from the
/// origin `o` holds it.
pub fn entry_node(cache: &[u32], e: usize, o: [i32; 3]) -> cast::Node {
    let w = e + node_cache::W_NODE as usize;
    cast::Node::of_idx(o, cache[w], [1, 2, 3].map(|a| cache[w + a] as i32))
}

/// Whether slot `slot` of `cache` holds a node. It does when its key is neither empty nor a
/// claim.
pub fn holds_node(cache: &[u32], slot: u32) -> bool {
    let key = cache[node_cache::key_at(slot) as usize];
    key != 0 && key != node_cache::CLAIMED && key & node_cache::CLAIM_MASK != node_cache::CLAIM_FAR
}

/// Reads back the words of `from`, which must allow copies from it (`COPY_SRC`).
pub fn read_buf(device: &wgpu::Device, queue: &wgpu::Queue, from: &wgpu::Buffer) -> Vec<u32> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: from.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut e = device.create_command_encoder(&Default::default());
    e.copy_buffer_to_buffer(from, 0, &staging, 0, from.size());
    queue.submit([e.finish()]);
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    wait(device);
    let v = staging.slice(..).get_mapped_range().unwrap();
    v.as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}

/// A texture of `layers` R32Uint layers at `size`, usable for `usage` and for copies both
/// ways, filled from `words` one layer after another.
pub fn texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
    layer_cnt: u32,
    usage: wgpu::TextureUsages,
    words: &[u32],
) -> wgpu::Texture {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: layer_cnt,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: usage | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    queue.write_texture(
        t.as_image_copy(),
        bytes(words),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * 4),
            rows_per_image: Some(size[1]),
        },
        t.size(),
    );
    t
}

/// Reads back a texture's words whole, one layer after another, without the padding a copy
/// adds to each row.
pub fn read_texture(device: &wgpu::Device, queue: &wgpu::Queue, t: &wgpu::Texture) -> Vec<u32> {
    use wgpu::util::DeviceExt as _;
    let [w, h, layers] = [t.width(), t.height(), t.depth_or_array_layers()];
    let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let staging = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytes(&vec![0u32; (row / 4 * h * layers) as usize]),
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
    });
    let mut e = device.create_command_encoder(&Default::default());
    e.copy_texture_to_buffer(
        t.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        t.size(),
    );
    queue.submit([e.finish()]);
    read_buf(device, queue, &staging)
        .chunks((row / 4) as usize)
        .flat_map(|r| r[..w as usize].to_vec())
        .collect()
}

/// A view of every layer of `t`, as a pass binds a texture array.
pub fn array_view(t: &wgpu::Texture) -> wgpu::TextureView {
    t.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// How many cases of each kind a test met. A test may claim a result only from the kinds it
/// met, so a kind it needs and met too few times fails it.
#[derive(Default, Debug)]
pub struct KindCount(BTreeMap<&'static str, u64>);

impl KindCount {
    pub fn new() -> KindCount {
        KindCount::default()
    }

    pub fn add(&mut self, k: &'static str, yes: bool) {
        self.count(k, u64::from(yes));
    }

    pub fn count(&mut self, k: &'static str, n: u64) {
        *self.0.entry(k).or_default() += n;
    }

    pub fn get(&self, k: &str) -> u64 {
        self.0.get(k).copied().unwrap_or(0)
    }

    /// A message for each kind in `least` met fewer times than the count it asks for.
    pub fn short(&self, least: &[(&str, u64)]) -> Vec<String> {
        least
            .iter()
            .filter(|(k, n)| self.get(k) < *n)
            .map(|(k, n)| format!("too few of {k}: {}, wanted {n}", self.get(k)))
            .collect()
    }

    /// Fails the test unless each kind in `what` was met at least once.
    pub fn hold(&self, what: &[&str]) {
        eprintln!("  KINDS {:?}", self.0);
        let short = self.short(&what.iter().map(|&k| (k, 1)).collect::<Vec<_>>());
        assert!(short.is_empty(), "{}", short.join("\n"));
    }
}

/// A light's box as the renderer lays it (pass/shadow.rs `aim`, `Shadow::starts`). The map is
/// `size` texels a side, its middle lies at the Q3 position `origin`, and it looks along `dir`.
/// A texel is `side` units wide, the near plane lies `back` units back along the light, and the
/// rays reach `t_max`. `corner` is the world texel index of the box's texel (0, 0), and
/// `reach_ndc` is the share of the box's half-width a receiver reads the map within.
pub struct LightBox {
    pub size: u32,
    pub origin: [i32; 3],
    pub dir: [f32; 3],
    pub side: f32,
    pub back: f32,
    pub t_max: f32,
    pub corner: [i32; 2],
    pub reach_ndc: f32,
}

impl LightBox {
    /// The light, and where its texels' rays start. Each anchor lies where its texel lies off
    /// the box's middle, rounded to a Q3 step with ties to even.
    pub fn lay(&self) -> (Light, StartGrid) {
        let dir = unit(self.dir);
        let a = cross(dir, [0.0, 0.0, 1.0]);
        let a = if a == [0.0; 3] {
            [1.0, 0.0, 0.0]
        } else {
            unit(a)
        };
        let across = a.map(|v| v * self.side);
        let down = unit(cross(dir, a)).map(|v| v * self.side);
        let half = self.size as f32 / 2.0;
        let l = Light {
            first: [0, 1, 2].map(|k| -half * (across[k] + down[k]) - self.back * dir[k]),
            t_max: self.t_max,
            across,
            down,
            dir,
            bias: [0.00025, 0.0005, 0.00075],
            corner: self.corner,
            reach_ndc: self.reach_ndc,
        };
        let base = self
            .corner
            .map(|v| v.div_euclid(light::ANCHOR) * light::ANCHOR);
        let q = (1u32 << FRAC_W) as f32;
        let anchor = |k: usize| {
            let w = [
                base[0] + (k % 2) as i32 * light::ANCHOR,
                base[1] + (k / 2) as i32 * light::ANCHOR,
            ];
            let (i, j) = (
                (w[0] - self.corner[0]) as f32,
                (w[1] - self.corner[1]) as f32,
            );
            let v = |a: usize| (l.first[a] + i * across[a] + j * down[a]) * q;
            (
                [0, 1, 2].map(|a| self.origin[a].wrapping_add(v(a).round_ties_even() as i32)),
                [0, 1, 2].map(|a| v(a) - v(a).round_ties_even()),
            )
        };
        let [a0, a1, a2, a3] = [0, 1, 2, 3].map(anchor);
        let starts = StartGrid {
            base,
            across: across.map(|v| light::fixed(v * q)),
            down: down.map(|v| light::fixed(v * q)),
            at: [a0.0, a1.0, a2.0, a3.0],
            frac: [a0.1, a1.1, a2.1, a3.1].map(|f| f.map(light::fixed)),
        };
        (l, starts)
    }
}

/// The margins a device may take for `light::margin` of a beam `width` wide, each named by its
/// rounding. The engine rounds the product and then adds, and a device may fuse the two into
/// one rounding (beam.light.wgsl `beam_hand`). The width comes from the host's word
/// (`Light::width`), so no other step rounds apart.
pub fn margins(width: f32) -> [(&'static str, f32); 2] {
    let fused = (2.0 * cast::SQRT_6).mul_add(width, light::margin(0.0));
    [
        ("the engine's", light::margin(width)),
        ("a fused sum's", fused),
    ]
}

pub fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|x| x / l)
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The ulps between `a` and `b`, by their order as floats.
pub fn ulps(a: f32, b: f32) -> u32 {
    let key = |v: f32| {
        let i = v.to_bits() as i32;
        if i < 0 {
            i32::MIN.wrapping_sub(i)
        } else {
            i
        }
    };
    key(a).abs_diff(key(b))
}

/// How many ulps a plane's t may move when the device works out the ray's direction `d` on an
/// axis itself. The device sums terms near 1 and lands a few ulps of 1 off, which is a large
/// share of a small `d`. Where `d` is so small that the share has no bound, the slack is as
/// many as a u32 holds. An axis the ray does not move along has none.
pub fn slack(d: f32) -> u32 {
    if d == 0.0 {
        0
    } else {
        (4.0 / d.abs()).ceil() as u32
    }
}

/// How many ulps of a device's rounding may carry a ray across an edge of a cell, beyond what
/// the rounding of its direction adds (`slack`).
pub const EDGE_ULPS: u32 = 16;

/// How many ulps the device's rounding may take between the planes of axes `a` and `b` along
/// the direction `dir`: `EDGE_ULPS` plus each axis's slack. A bound that the origin or the
/// ray's reach sets is given as `None` and adds no slack.
fn edge_room(dir: [f32; 3], a: Option<usize>, b: Option<usize>) -> u32 {
    let s = |a: Option<usize>| a.map_or(0, |a| slack(dir[a]));
    EDGE_ULPS.saturating_add(s(a)).saturating_add(s(b))
}

/// The axes across which the device, as it rounds the ray, may make entry `e` along `dir`.
/// They are the entry's own axis, and each other axis the ray moves along whose near plane lies
/// within `edge_room` of the plane the entry crosses. Such an entry lies at an edge between the
/// two planes.
pub fn edge_axes(e: &cast::Entry, dir: [f32; 3]) -> Vec<u32> {
    let axis = e.axis as usize;
    (0..3)
        .filter(|&a| {
            a == axis
                || (dir[a] != 0.0
                    && ulps(e.near[a], e.near[axis]) <= edge_room(dir, Some(a), Some(axis)))
        })
        .map(|a| a as u32)
        .collect()
}

/// Whether the device's rounding may decide entry `e` along `dir` otherwise than the engine
/// does. It may where the entry's start lies within `edge_room` of its end, or where the plane
/// it enters across lies within `edge_room` of another axis's near plane (`edge_axes`).
pub fn at_edge(e: &cast::Entry, dir: [f32; 3]) -> bool {
    let moves = |a: usize| dir[a] != 0.0;
    let enter = (0..3).find(|&a| moves(a) && e.near[a] == e.span[0]);
    let leave = (0..3).find(|&a| moves(a) && e.far[a] == e.span[1]);
    ulps(e.span[0], e.span[1]) <= edge_room(dir, enter, leave) || edge_axes(e, dir).len() > 1
}

/// Whether the `edges` of a run's `rays` that the device may decide at an edge (`at_edge`)
/// are few enough. A run may have one in a thousand at most.
pub fn edges_within(edges: u64, rays: u64) -> bool {
    edges * 1000 <= rays
}

/// A hash of two words, for a case's choices.
pub fn mix(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ h >> 12
}

/// A tally of the cases a test compared: how many cases and words it compared, and the name
/// and count of differing words of each case that differs. Under a mutant sweep, the first
/// case that differs stops the test.
#[derive(Default)]
pub struct Tally {
    pub cases: u32,
    pub words: u64,
    pub differ: Vec<(String, u64)>,
}

impl Tally {
    /// Compares `have` with `want` word for word as the case named `case`. Where word `k`
    /// differs, `what(k)` names it in the message.
    pub fn compare(
        &mut self,
        case: &str,
        want: &[u32],
        have: &[u32],
        what: impl Fn(usize) -> String,
    ) {
        self.compare_by(case, want, have, what, |_, w, h| w == h);
    }

    /// `compare`, where word `k` counts as alike when `alike(k, want, have)` holds.
    pub fn compare_by(
        &mut self,
        case: &str,
        want: &[u32],
        have: &[u32],
        what: impl Fn(usize) -> String,
        alike: impl Fn(usize, u32, u32) -> bool,
    ) {
        assert_eq!(want.len(), have.len(), "{case}: lengths");
        self.cases += 1;
        self.words += want.len() as u64;
        let mut n = 0u64;
        for (k, (w, h)) in want.iter().zip(have).enumerate() {
            if !alike(k, *w, *h) {
                if n < 4 {
                    eprintln!("{case}: {} rust {w:#010x} hand {h:#010x}", what(k));
                }
                n += 1;
            }
        }
        if n > 0 {
            eprintln!("{case}: {n} of {} words differ", want.len());
            assert!(!mutant(), "{case}: differs");
            self.differ.push((case.to_string(), n));
        } else {
            eprintln!("{case}: {} words alike", want.len());
        }
    }

    pub fn end(&self, what: &str) {
        eprintln!(
            "{what}: {} cases, {} words, {} cases differ",
            self.cases,
            self.words,
            self.differ.len()
        );
        assert!(self.differ.is_empty(), "{what}: {:?} differ", self.differ);
    }
}
