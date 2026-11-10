// xpute-triangle/lib.rs

//! The smallest core that draws, meant to be read beside the page and the app
//! that run it (ts/dev/triangle). The UI opens a surface and sizes it, and from
//! then on each turn the GPU admits draws one frame of a triangle that turns.
//! The same crate is a page's WebAssembly module and a device's library.

#[path = "machine.spec.rs"]
mod machine;

use core::mem::take;

use machine::{GPU_ASK, MACHINE};
use xpute_core::abi::handle::{handle_slot, Table};
use xpute_core::entry::BootInfo;
use xpute_core::global::Global;
use xpute_core::ipc::door::{Outcome, Submission};
use xpute_core::ipc::stream::{Stream, WORD};
use xpute_core::machine as core_machine;
use xpute_core::mem;
use xpute_gpu::device;
use xpute_gpu::encode::{
    call, load_acquire, ColorAttachment, ColorTargetState, FragmentState, Gpu, Id, PrimitiveState,
    RenderPassDescriptor, RenderPipelineDescriptor, VertexState, MULTISAMPLE_NONE,
};
use xpute_gpu::enums::{
    CanvasAlphaMode, CullMode, FrontFace, LoadOp, PrimitiveTopology, StoreOp, TextureFormat,
};
use xpute_gpu::spec::{COLOR_WRITE, STATUS, TEXTURE_USAGE};
use xpute_gpu::submission::{decode, Submission as GpuSubmission, Surface};
use xpute_gpu::CoreConfig;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

/// The major this program gives the GPU's commands, which its UI names too
/// (ts/dev/triangle).
const GPU_MAJOR: u32 = 0x10;

const CONFIG: CoreConfig = CoreConfig {
    frame_latency: 2,
    upload_chunk_bytes: 1 << 16,
};

xpute_gpu::core!(MACHINE, 0, entry, CONFIG, GPU_ASK);

/// The vertices turn by the frame's number, which each draw hands the shader
/// as its first instance.
const SHADER: &str = "
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) frame: u32)
    -> @builtin(position) vec4f {
  let a = f32(frame) * 0.03 + f32(v) * 2.0944;
  return vec4f(0.7 * cos(a), 0.7 * sin(a), 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4f { return vec4f(1.0, 0.45, 0.1, 1.0); }
";

/// The device's block sits in the stream's own words, inside the core's memory
/// where the runtime can write it.
const DEVICE_WORDS: usize = Stream::OWN_WORDS;
const HEAD_WORDS: usize = DEVICE_WORDS + device::WORDS;

struct Triangle {
    stream: Stream,
    gpu: Gpu<8>,
    surfaces: Table<(), 2>,
    /// The surface open, or 0.
    surface: u32,
    size: [u32; 2],
    resized: bool,
    configured: bool,
    /// Whether a device answered and its block is yet to be asked for.
    ask_device: bool,
    pipeline: Option<Id>,
    frame: u32,
}

static TRIANGLE: Global<Triangle> = Global::new();

impl Triangle {
    fn new() -> Triangle {
        let stream = MACHINE.stream.expect("the machine has a stream");
        Triangle {
            // SAFETY: the machine's stream range, which only this core writes.
            stream: unsafe {
                Stream::new(
                    mem::core(),
                    MACHINE.stream_at() as usize,
                    stream.bytes as usize / WORD,
                    HEAD_WORDS,
                )
            },
            gpu: Gpu::new(),
            surfaces: Table::new(),
            surface: 0,
            size: [0; 2],
            resized: false,
            configured: false,
            ask_device: false,
            pipeline: None,
            frame: 0,
        }
    }

    fn apply(&mut self, s: Submission) -> Outcome {
        let Submission::Program(cmd, pkt) = s else {
            return Ok(Some(0));
        };
        match decode(GPU_MAJOR, cmd, pkt)? {
            Some(GpuSubmission::Ready) => self.ask_device = true,
            // A new UI's surface replaces the last one's.
            Some(GpuSubmission::Surface(Surface::Open)) => {
                self.surfaces.remove(self.surface);
                self.surface = self.surfaces.insert(()).unwrap_or(0);
                self.configured = false;
                return Ok(Some(self.surface as i32));
            }
            Some(GpuSubmission::Surface(Surface::Resize(c))) => {
                if c.surface == self.surface {
                    self.size = [c.width, c.height];
                    self.resized = true;
                }
            }
            Some(GpuSubmission::Surface(Surface::Close(c))) => {
                if c.surface == self.surface {
                    self.surfaces.remove(self.surface);
                    self.surface = 0;
                }
            }
            None => return Err(MarshalError::new(Errno::ENOSYS)),
        }
        Ok(Some(0))
    }

    /// The device's block, which only the runtime writes.
    fn block(&self) -> &'static mut [u32] {
        let at = self.stream.at() + DEVICE_WORDS * WORD;
        // SAFETY: the words `new` left between the stream's own and its records.
        unsafe { core::slice::from_raw_parts_mut(mem::at::<u32>(at), device::WORDS) }
    }

    fn pipeline(&mut self) -> Option<Id> {
        if self.pipeline.is_none() {
            let word = load_acquire(&self.block()[device::FORMAT_AT]);
            let format = *TextureFormat::ALL.get(word as usize)?;
            let s = &mut self.stream;
            let module = self.gpu.shader_module(s, SHADER)?;
            self.pipeline = self.gpu.render_pipeline(
                s,
                &RenderPipelineDescriptor {
                    layout: None,
                    vertex: VertexState {
                        module,
                        entry: "vs",
                        buffers: &[],
                    },
                    primitive: PrimitiveState {
                        topology: PrimitiveTopology::triangle_list,
                        strip_index_format: None,
                        front_face: FrontFace::ccw,
                        cull: CullMode::none,
                        unclipped_depth: false,
                    },
                    depth_stencil: None,
                    multisample: MULTISAMPLE_NONE,
                    fragment: Some(FragmentState {
                        module,
                        entry: "fs",
                        targets: &[ColorTargetState {
                            format,
                            blend: None,
                            write_mask: COLOR_WRITE::ALL,
                        }],
                    }),
                },
            );
            self.gpu.destroy(s, module);
        }
        self.pipeline
    }

    fn draw(&mut self) -> Option<()> {
        let pipeline = self.pipeline()?;
        let surface = handle_slot(self.surface);
        let s = &mut self.stream;
        if !self.configured {
            call::configure(
                s,
                surface,
                None,
                TEXTURE_USAGE::RENDER_ATTACHMENT,
                CanvasAlphaMode::opaque,
            );
            self.configured = true;
        }
        if take(&mut self.resized) {
            call::resize(s, surface, self.size);
        }
        let view = self.gpu.current(s, surface)?;
        let encoder = self.gpu.encoder(s)?;
        let pass = self.gpu.render_pass(
            s,
            encoder,
            &RenderPassDescriptor {
                colors: &[ColorAttachment {
                    view,
                    resolve: None,
                    load: LoadOp::clear,
                    store: StoreOp::store,
                    clear: [0.05, 0.05, 0.12, 1.0],
                }],
                depth_stencil: None,
                timestamps: None,
                occlusion: None,
            },
        )?;
        call::set_pipeline(s, pass, pipeline);
        call::draw(s, pass, 3, 1, 0, self.frame);
        self.gpu.end(s, pass);
        self.gpu.submit(s, &[encoder]);
        self.gpu.destroy(s, view);
        self.frame = self.frame.wrapping_add(1);
        Some(())
    }

    /// Writes this turn's records, while the stream is the core's.
    fn record(&mut self) {
        if !self.stream.is_free() {
            return;
        }
        let status = load_acquire(&self.block()[device::STATUS_AT]);
        // The device went with every object on it, so the handles start over.
        if status == STATUS::LOST && self.pipeline.is_some() {
            (self.gpu, self.pipeline, self.configured) = (Gpu::new(), None, false);
        }
        if take(&mut self.ask_device) {
            let block = self.block();
            call::device(&mut self.stream, block);
        }
        let sized = self.size != [0; 2];
        if status == STATUS::READY && self.surface != 0 && sized && admit_frame() {
            self.draw();
        }
        self.stream.publish();
    }
}

/// A turn applies what the UI sent and draws. While a surface is open the core
/// wants the next turn at once, which the runtime's pacing grants.
fn entry(_deadline_ms: f64, _boot: fn() -> BootInfo) -> f64 {
    if !core_machine::booted() {
        core_machine::boot(&MACHINE);
    }
    let t = TRIANGLE.get(Triangle::new);
    let wake = core_machine::door().turn(|s| t.apply(s));
    t.record();
    if t.surface != 0 {
        0.0
    } else {
        wake
    }
}
