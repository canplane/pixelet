//! The device every test of a pass by hand runs it on (tests/fold.rs, tests/fill.rs and the
//! rest): its buffers and textures, a module taken as the renderer assembles it or from a file
//! a mutant sweep names, and a wait for the device that gives up, as a shader by hand may loop
//! on; and what the tests share besides: the kinds of
//! case each met, a light's box as a host lays it, a hash and a few vectors.

#![allow(dead_code, reason = "each test reads its own of it")]

use std::collections::BTreeMap;
use std::time::Duration;

use pxl_engine::light::{self, Light, Starts};
use pxl_engine::FRAC_W;

/// How long a test waits on the device for one submission before it fails.
const WAIT: Duration = Duration::from_secs(60);

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
