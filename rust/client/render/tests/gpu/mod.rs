//! The device every test of a pass by hand runs it on (tests/fill.rs and the
//! rest): its buffers and textures, a module taken as the renderer assembles it or from a file
//! a mutant sweep names, and a wait for the device that gives up, as a shader by hand may loop
//! on; and what the tests share besides: the light's beam as the cast runs it, the kinds of
//! case each met, a light's box as a host lays it, a hash and a few vectors.

#![allow(dead_code, reason = "each test reads its own of it")]

use std::collections::BTreeMap;
use std::time::Duration;

use pxl_engine::answer::Kind;
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::{cast, node_cache, ray, FRAC_W};
use pxl_render::frame::shader::record::{
    compute, src, COMPUTE_QUEUE, COMPUTE_QUEUE_ASK, COMPUTE_QUEUE_ASK_CORNERS,
    COMPUTE_QUEUE_ASK_CORNERS_PARKED, COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
    COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED, COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
    COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED, COMPUTE_QUEUE_ASK_PARKED, COMPUTE_QUEUE_ASK_SAMPLES,
    COMPUTE_QUEUE_ASK_SAMPLES_PARKED, COMPUTE_QUEUE_CORNERS, COMPUTE_QUEUE_CORNERS_PARKED,
    COMPUTE_QUEUE_LAST, COMPUTE_QUEUE_LAST_CORNERS, COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
    COMPUTE_QUEUE_LAST_LIGHT_SAMPLES, COMPUTE_QUEUE_LAST_SAMPLES, COMPUTE_QUEUE_LIGHT_CORNERS,
    COMPUTE_QUEUE_LIGHT_CORNERS_PARKED, COMPUTE_QUEUE_LIGHT_SAMPLES,
    COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED, COMPUTE_QUEUE_PARKED, COMPUTE_QUEUE_SAMPLES,
    COMPUTE_QUEUE_SAMPLES_PARKED,
};

/// How long a test waits on the device for one submission before it fails.
const WAIT: Duration = Duration::from_secs(60);
/// The frames a run may take to ask what its rays need (`Gpu::run`).
const FRAMES_TO_ASK: u32 = 64;

/// Whether a mutant sweep runs this test: the file `MUT_WGSL` names stands for the module
/// that holds the entry, and the test stops at its first case that differs.
pub fn mutant() -> bool {
    std::env::var_os("MUT_WGSL").is_some()
}

/// The module that holds `entry`: as assembled, or the file `MUT_WGSL` names where it
/// holds the entry.
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
}

impl Gpu {
    /// A device of the adapter's own limits; none where no adapter answers.
    pub fn new() -> Option<Gpu> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .unwrap();
        Some(Gpu { device, queue })
    }

    /// Every submission done, or the test failed.
    pub fn wait(&self) {
        wait(&self.device);
    }

    pub fn buffer(&self, words: &[u32], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        use wgpu::util::DeviceExt as _;
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytes(words),
                usage: usage | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            })
    }

    pub fn read(&self, from: &wgpu::Buffer) -> Vec<u32> {
        read_buffer(&self.device, &self.queue, from)
    }

    /// The pipeline of `entry` in `assembled`, or in the mutant's module (`source`), its
    /// group laid out as the module reads it; a module the device refuses fails the test.
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

    /// `groups` workgroups of `p` over `binds`, waited for; a command buffer the device
    /// stopped short fails the test.
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
        // A word the same command buffer clears after the pass: one Metal aborts mid-way runs
        // nothing after, and wgpu counts it done all the same.
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

    /// The light's beam rays' run as the cast runs it over the map `size` texels a side
    /// (corners.wgsl over beam.light.wgsl), every beam ray of the whole map listed
    /// (light_list.wgsl `light_item_at`), as `corners` runs it. `world` binds what its passes
    /// read: the list at 8, the node cache `table` at 9, the head at 11.
    pub fn light_beam(
        &self,
        world: &[(u32, wgpu::BindingResource)],
        list: &wgpu::Buffer,
        table: &wgpu::Buffer,
        size: u32,
    ) {
        let mut words = self.read(list);
        list_every(&mut words, size, light::LISTED_CORNERS, 1);
        self.queue.write_buffer(list, 0, bytes(&words));
        self.corners(true, world, table);
    }

    /// The holes' run as the cast runs it over one observer's holes (pxl-render gpu/ray/cast.rs
    /// `walk_run`): the two walks over the node cache alone and the asks after each, the stops
    /// copied into `table` and the rays parked into `pool`, a buffer and its word (pxl-engine
    /// `ray::POOL_CNT`), as the cast copies them, and the last walk. `all` binds what any of its
    /// passes reads but the queue's words: the eye at 0, the world's at 1, 2 and 4 to 7, the rays
    /// or the map at 3, the observer's list at 8, the node cache at 9, the observer's head at 11;
    /// each pass takes those its entry reads.
    pub fn holes(
        &self,
        all: &[(u32, wgpu::BindingResource)],
        table: &wgpu::Buffer,
        pool: (&wgpu::Buffer, u32),
    ) {
        self.run(
            [
                COMPUTE_QUEUE,
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
                COMPUTE_QUEUE_LAST,
            ],
            [
                &[0, 3, 8, 9, 10, 11],
                &[0, 1, 2, 4, 5, 6, 7, 9, 10, 11],
                &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11],
            ],
            all,
            table,
            None,
            Some(pool),
        );
    }

    /// An observer's beam rays' run as the cast runs it (pxl-render gpu/ray/cast.rs
    /// `walk_run`), the eye's or the `light`'s, as `samples` runs the samples'.
    pub fn corners(&self, light: bool, all: &[(u32, wgpu::BindingResource)], table: &wgpu::Buffer) {
        let ids = if light {
            [
                COMPUTE_QUEUE_LIGHT_CORNERS,
                COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
                COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED,
                COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
            ]
        } else {
            [
                COMPUTE_QUEUE_CORNERS,
                COMPUTE_QUEUE_ASK_CORNERS,
                COMPUTE_QUEUE_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_CORNERS_PARKED,
                COMPUTE_QUEUE_LAST_CORNERS,
            ]
        };
        self.lattice(ids, light, all, table, None);
    }

    /// An observer's samples' run as the cast runs it (pxl-render gpu/ray/cast.rs `walk_run`),
    /// the eye's or the `light`'s: the two walks over the node cache alone and the asks after
    /// each, and the last walk. `all` binds what any of its passes reads but the queue's words:
    /// the eye at 0, the world's at 1, 2 and 4 to 7, the observer's list at 8, the node cache at
    /// 9, the light's head at 11, each pass taking those its entry reads.
    pub fn samples(&self, light: bool, all: &[(u32, wgpu::BindingResource)], table: &wgpu::Buffer) {
        self.samples_asked(light, all, table, None);
    }

    /// `samples`, its last walk, which asks the world, over the node cache `asked` where given
    /// in place of `table`, which the walks before it and their asks read and write.
    pub fn samples_asked(
        &self,
        light: bool,
        all: &[(u32, wgpu::BindingResource)],
        table: &wgpu::Buffer,
        asked: Option<&wgpu::Buffer>,
    ) {
        let ids = if light {
            [
                COMPUTE_QUEUE_LIGHT_SAMPLES,
                COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
                COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED,
                COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
            ]
        } else {
            [
                COMPUTE_QUEUE_SAMPLES,
                COMPUTE_QUEUE_ASK_SAMPLES,
                COMPUTE_QUEUE_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_SAMPLES_PARKED,
                COMPUTE_QUEUE_LAST_SAMPLES,
            ]
        };
        self.lattice(ids, light, all, table, asked);
    }

    /// A lattice's run of passes `ids`, the eye's or the `light`'s, over `table` and `asked`
    /// (`samples_asked`).
    fn lattice(
        &self,
        ids: [u32; 5],
        light: bool,
        all: &[(u32, wgpu::BindingResource)],
        table: &wgpu::Buffer,
        asked: Option<&wgpu::Buffer>,
    ) {
        // The light's walks read its head; no lattice's asks do, nor the eye's walks.
        let head: &[u32] = if light { &[11] } else { &[] };
        let walk = [&[0, 8, 9, 10][..], head].concat();
        let last = [&[0, 1, 2, 4, 5, 6, 7, 8, 9][..], head].concat();
        let keep = [&walk[..], &[0, 1, 2, 4, 5, 6, 7, 9, 10], &last[..]];
        self.run(ids, keep, all, table, asked, None);
    }

    /// A run's passes `ids` (its walk, its asks, its walk again, their asks and its last walk)
    /// over `all` and the queue's words, the walks, the asks and the last walk each taking the
    /// bindings `keep` names for it, the last walk's node cache `asked` where given; where
    /// `pool`, the holes' run's, its stops and its parked rays copied as `holes` says. A run that
    /// asks past `ray::ASKS_A_FRAME` ends rays as lacks, so it runs again over the node cache it
    /// left, as the frames after do, until one lacks no ask: its words are what a test reads.
    fn run(
        &self,
        [walk_id, ask_id, walk_parked_id, ask_parked_id, last_id]: [u32; 5],
        [walk_keep, ask_keep, last_keep]: [&[u32]; 3],
        all: &[(u32, wgpu::BindingResource)],
        table: &wgpu::Buffer,
        asked: Option<&wgpu::Buffer>,
        pool: Option<(&wgpu::Buffer, u32)>,
    ) {
        let queue = self.buffer(
            &vec![0u32; ray::QUEUE_WORDS as usize],
            wgpu::BufferUsages::STORAGE,
        );
        let pipeline = |id| {
            let (shader, entry) = compute(id);
            self.pipeline(src(shader), entry)
        };
        let take = |keep: &[u32]| {
            let mut binds: Vec<_> = all
                .iter()
                .filter(|(b, _)| keep.contains(b))
                .cloned()
                .collect();
            if keep.contains(&10) {
                binds.push((10, queue.as_entire_binding()));
            }
            binds
        };
        let [walk, ask, walk_parked, ask_parked, last] =
            [walk_id, ask_id, walk_parked_id, ask_parked_id, last_id].map(pipeline);
        let walk_binds = take(walk_keep);
        let ask_binds = take(ask_keep);
        let mut last_binds = take(last_keep);
        if let Some(asked) = asked {
            for (b, resource) in &mut last_binds {
                if *b == 9 {
                    *resource = asked.as_entire_binding();
                }
            }
        }
        let groups = [ray::QUEUE_GROUPS, 1, 1];
        let word = |w: u32| u64::from(w) * 4;
        for _ in 0..FRAMES_TO_ASK {
            // Each run a frame of its own: its asks none and the last walk's first taken, as the
            // host and the asks set them each frame it casts.
            let none = [0u32; (node_cache::ASKED + 1 - node_cache::LAST_TAKEN) as usize];
            for t in [Some(table), asked].into_iter().flatten() {
                self.queue
                    .write_buffer(t, word(node_cache::LAST_TAKEN), bytes(&none));
            }
            self.queue.write_buffer(&queue, 0, bytes(&ray::QUEUE_EMPTY));
            self.dispatch(&walk, &walk_binds, groups);
            self.dispatch(&ask, &ask_binds, groups);
            self.dispatch(&walk_parked, &walk_binds, groups);
            self.dispatch(&ask_parked, &ask_binds, groups);
            if let Some((buf, at)) = pool {
                let q = self.read(&queue);
                let (h, p) = (ray::QUEUE_HITS as usize, ray::QUEUE_PARKED as usize);
                self.queue
                    .write_buffer(table, word(node_cache::HITS_ARGS), bytes(&q[h..h + 4]));
                self.queue.write_buffer(buf, word(at), bytes(&q[p..p + 1]));
            }
            self.dispatch(&last, &last_binds, groups);
            let done = self.read(asked.unwrap_or(table));
            if done[node_cache::ASKED as usize] <= ray::ASKS_A_FRAME {
                return;
            }
        }
        panic!("a run still asks past a frame's asks after {FRAMES_TO_ASK} frames");
    }
}

/// List `which` of the light's list `words` over a map `size` texels a side holding every item
/// of its level, as a cast that claims every tile lays it (light_propagate.wgsl `enlist`); and
/// the workgroups a pass takes it over whose entries are `per` items a side
/// (light_list.wgsl `light_item`).
pub fn list_every(words: &mut [u32], size: u32, which: u32, per: u32) -> [u32; 2] {
    let across = if which == light::LISTED_CORNERS {
        light::beam_across(size)
    } else {
        light::tiles(size)
    };
    let at = light::listed_at(size, which) as usize;
    words[at] = across * across;
    for k in 0..across * across {
        words[at + 1 + k as usize] = ((k / across) << 16) | (k % across);
    }
    let threads = across * across * per * per;
    [
        light::LIST_ACROSS,
        threads.div_ceil(light::LIST_ACROSS * light::LISTED_LANES),
    ]
}

/// Every submission on `device` done, or the test failed.
fn wait(device: &wgpu::Device) {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(WAIT),
        })
        .unwrap_or_else(|e| panic!("the device did not finish in {WAIT:?}: {e:?}"));
}

/// The words of `from`, a buffer that may be copied from.
/// Lays in the node cache's words `cache` a drawable entry in slot `slot` (pxl-engine
/// `node_cache`, `record`): the node `n` as a walk from the origin `o` holds it, kept by a walk
/// of frame 1 at the node's own zoom, answered `kind` and `asset`, what draws it `state`.
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
    cache[node_cache::key_at(slot) as usize] = node_cache::PASSES | zooms << node_cache::TAG_BITS;
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

/// Lays the node `n`, as a walk from the origin `o` holds it, answered `kind` and `asset`, in
/// the first free slot of its bucket of the node cache's words `cache`, as a walk that asked it
/// keeps it (pxl-render ask.wgsl `keep`), what draws it none: its slot, none where it is kept
/// already or its bucket is full.
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
        if entry_held(cache, slot) {
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
/// origin `o` holds it (`cast::Node::of_idx`).
pub fn entry_node(cache: &[u32], e: usize, o: [i32; 3]) -> cast::Node {
    let w = e + node_cache::W_NODE as usize;
    cast::Node::of_idx(o, cache[w], [1, 2, 3].map(|a| cache[w + a] as i32))
}

/// Whether slot `slot` of `cache` holds a node: a key that is no claim.
pub fn entry_held(cache: &[u32], slot: u32) -> bool {
    let key = cache[node_cache::key_at(slot) as usize];
    key != 0 && key != node_cache::CLAIMED && key & node_cache::CLAIM_MASK != node_cache::CLAIM_FAR
}

pub fn read_buffer(device: &wgpu::Device, queue: &wgpu::Queue, from: &wgpu::Buffer) -> Vec<u32> {
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

/// A texture of `layers` R32Uint layers at `size`, for `usage` and copies either way, laid
/// with `words`, a layer after another.
pub fn texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
    layers: u32,
    usage: wgpu::TextureUsages,
    words: &[u32],
) -> wgpu::Texture {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: layers,
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

/// A texture's words read back whole, a layer after another, a row's padding dropped.
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
    read_buffer(device, queue, &staging)
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

/// How many of each kind of case a test met, for what it may say: a kind it needs and met
/// too few of fails it.
#[derive(Default, Debug)]
pub struct Kinds(BTreeMap<&'static str, u64>);

impl Kinds {
    pub fn new() -> Kinds {
        Kinds::default()
    }

    /// One more of `k` where `yes`.
    pub fn add(&mut self, k: &'static str, yes: bool) {
        self.count(k, u64::from(yes));
    }

    pub fn count(&mut self, k: &'static str, n: u64) {
        *self.0.entry(k).or_default() += n;
    }

    pub fn get(&self, k: &str) -> u64 {
        self.0.get(k).copied().unwrap_or(0)
    }

    /// Each kind of `least` met fewer times than it asks, told.
    pub fn short(&self, least: &[(&str, u64)]) -> Vec<String> {
        least
            .iter()
            .filter(|(k, n)| self.get(k) < *n)
            .map(|(k, n)| format!("too few of {k}: {}, wanted {n}", self.get(k)))
            .collect()
    }

    /// Each of `what` met at least once, or the test fails.
    pub fn hold(&self, what: &[&str]) {
        eprintln!("  KINDS {:?}", self.0);
        let short = self.short(&what.iter().map(|&k| (k, 1)).collect::<Vec<_>>());
        assert!(short.is_empty(), "{}", short.join("\n"));
    }
}

/// A light's box as a host lays it (pxl-render gpu/shadow.rs `aim`, `Shadow::starts`): the map
/// `size` texels a side, its middle at the Q3 position `origin`, looking along `dir`, a texel
/// `side` units, its near plane `back` units back along the light, its rays reaching `t_max`,
/// its corner and its receivers' reach.
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
    /// The light, and where its texels' rays start (`light::Starts`): each anchor where its
    /// texel lies off the middle, rounded to a Q3 step, half to even.
    pub fn lay(&self) -> (Light, Starts) {
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
        let starts = Starts {
            base,
            across: across.map(|v| light::fixed(v * q)),
            down: down.map(|v| light::fixed(v * q)),
            at: [a0.0, a1.0, a2.0, a3.0],
            frac: [a0.1, a1.1, a2.1, a3.1].map(|f| f.map(light::fixed)),
        };
        (l, starts)
    }
}

/// The margins a device may take for `light::margin` of a beam `width` wide, a rounding each,
/// named: the engine's, and the product fused into the sum (beam.light.wgsl `beam_hand`). The
/// width is the host's word (`Light::width`), so nothing else rounds apart.
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

/// The ulps a plane's t may move by as the device works out the ray's way `d` along an axis
/// itself: a sum of terms about 1, a few ulps of 1 off, is a large share of a small way. As
/// many as a u32 holds where the way is so small its share has no bound.
pub fn slack(d: f32) -> u32 {
    if d == 0.0 {
        0
    } else {
        (4.0 / d.abs()).ceil() as u32
    }
}

/// The ulps within which a device's rounding of a ray may take it across an edge of a cell,
/// past what its way's own rounding takes (`slack`).
pub const EDGE_ULPS: u32 = 16;

/// The room the device's rounding has between the planes of axes `a` and `b` along the way
/// `dir`: `EDGE_ULPS` and each axis's slack; a bound the origin or the ray's reach sets moves by
/// none.
fn edge_room(dir: [f32; 3], a: Option<usize>, b: Option<usize>) -> u32 {
    let s = |a: Option<usize>| a.map_or(0, |a| slack(dir[a]));
    EDGE_ULPS.saturating_add(s(a)).saturating_add(s(b))
}

/// The axes entry `e` along the way `dir` may enter across as the device rounds the ray: its
/// own, and each other axis the ray moves along whose near plane lies within the room of the
/// plane it enters across (`edge_room`), at an edge between the two.
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

/// Whether the device's rounding may decide entry `e` otherwise along the way `dir`: where it
/// enters lies within the room of where it leaves, or the plane it enters across within the
/// room of another axis's near plane (`edge_axes`).
pub fn at_edge(e: &cast::Entry, dir: [f32; 3]) -> bool {
    let moves = |a: usize| dir[a] != 0.0;
    let enter = (0..3).find(|&a| moves(a) && e.near[a] == e.span[0]);
    let leave = (0..3).find(|&a| moves(a) && e.far[a] == e.span[1]);
    ulps(e.span[0], e.span[1]) <= edge_room(dir, enter, leave) || edge_axes(e, dir).len() > 1
}

/// Whether `edges` of a run's `rays` decided at an edge as the device rounds them (`at_edge`)
/// are few enough: one in a thousand at most.
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

/// The cases that differ, each told by its name, how many words it compared and how many
/// differ: a sweep's mutant stops at its first.
#[derive(Default)]
pub struct Tally {
    pub cases: u32,
    pub words: u64,
    pub differ: Vec<(String, u64)>,
}

impl Tally {
    /// `have` against `want`, word for word, named `case`; `what` names word `k` where one
    /// differs.
    pub fn compare(
        &mut self,
        case: &str,
        want: &[u32],
        have: &[u32],
        what: impl Fn(usize) -> String,
    ) {
        self.compare_by(case, want, have, what, |_, w, h| w == h);
    }

    /// `compare`, word `k` alike where `alike(k, want, have)`.
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
