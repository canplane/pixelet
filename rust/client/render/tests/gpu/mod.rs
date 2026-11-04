//! The device a test of a pass by hand runs it on (tests/fold.rs, tests/fill.rs and the
//! rest): its buffers, a module taken as the renderer assembles it or from a file a
//! mutant sweep names, and a wait for the device that gives up, as a shader by hand may
//! loop on.

#![allow(dead_code, reason = "each test reads its own of it")]

use std::time::Duration;

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
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(WAIT),
            })
            .unwrap_or_else(|e| panic!("the device did not finish in {WAIT:?}: {e:?}"));
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
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: from.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut e = self.device.create_command_encoder(&Default::default());
        e.copy_buffer_to_buffer(from, 0, &staging, 0, from.size());
        self.queue.submit([e.finish()]);
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        self.wait();
        let v = staging.slice(..).get_mapped_range().unwrap();
        v.as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect()
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

    /// `groups` workgroups of `p` over `binds`, waited for.
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
        let mut e = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            pass.set_pipeline(p);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(groups[0], groups[1], groups[2]);
        }
        self.queue.submit([e.finish()]);
        self.wait();
    }
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
