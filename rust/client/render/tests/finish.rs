//! This test draws the finish as the renderer does (pass/finish.rs), from its shader module as
//! build.rs assembles it. The world's finish draws the canvas and the picture in one pass, and
//! both must equal what a character surface's finish draws onto the canvas alone. A frame not
//! drawn whole shows the picture first and then finishes only its damaged tiles again. Both
//! targets must still equal what the whole frame drew.

mod gpu;

use gpu::{bytes, read_buf, Gpu};
use pxl_render::frame::shader::record::{src, SHADER_SCREEN_FINISH, SHADER_SCREEN_SHOW};
/// A screen tile's side, counted in the world's pixels.
use pxl_render::frame::shader::word::screen::TILE as SIDE;
use pxl_render::frame::shader::word::{frame, FRAME_FLOATS};

/// The sizes of the world and the canvas in pixels. The canvas is twice the world, as when the
/// renderer draws the world at a share of the display's pixels.
const WORLD: [u32; 2] = [32, 24];
const CANVAS: [u32; 2] = [64, 48];
const CANVAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

struct Finish<'a> {
    gpu: &'a Gpu,
    world: wgpu::TextureView,
    depth: wgpu::TextureView,
    frame: wgpu::Buffer,
    changed_at: wgpu::Buffer,
    tiles: wgpu::Buffer,
}

fn texture(
    gpu: &Gpu,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn module(gpu: &Gpu, shader: u32) -> wgpu::ShaderModule {
    gpu.device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(src(shader).into()),
        })
}

/// The pipeline of `shader` that draws into `target_cnt` color targets, and into the screen's
/// depth when `depth` is set.
fn pipeline(gpu: &Gpu, shader: u32, target_cnt: usize, depth: bool) -> wgpu::RenderPipeline {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = module(gpu, shader);
    let target = Some(wgpu::ColorTargetState {
        format: CANVAS_FORMAT,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    });
    let colors = vec![target; target_cnt];
    let p = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: depth.then(|| wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &colors,
            }),
            multiview_mask: None,
            cache: None,
        });
    if let Some(e) = pollster::block_on(scope.pop()) {
        panic!("shader {shader} with {target_cnt} targets refused: {e}");
    }
    p
}

impl<'a> Finish<'a> {
    /// Makes a world whose color differs from texel to texel, and a depth cleared to one value.
    fn new(gpu: &'a Gpu) -> Finish<'a> {
        let use_ = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let world = texture(gpu, WORLD, wgpu::TextureFormat::Rgba16Float, use_);
        // `half(n)` encodes n/8 exactly as a half float. The bias puts 1.0 at 0x3c00, and
        // dividing by 8 lowers the exponent by 3.
        let half = |eighths: u32| -> u16 {
            match eighths {
                0 => 0,
                n => {
                    let e = 31 - n.leading_zeros();
                    let m = (n << (10 - e)) & 0x3ff;
                    (((15 + e - 3) << 10) | m) as u16
                }
            }
        };
        let texels: Vec<u16> = (0..WORLD[0] * WORLD[1])
            .flat_map(|k| {
                let (x, y) = (k % WORLD[0], k / WORLD[0]);
                [half(x % 9), half(y % 9), half((x + y) % 7), half(8)]
            })
            .collect();
        let words: Vec<u32> = texels
            .chunks(2)
            .map(|p| u32::from(p[0]) | u32::from(p[1]) << 16)
            .collect();
        gpu.queue.write_texture(
            world.as_image_copy(),
            bytes(&words),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WORLD[0] * 8),
                rows_per_image: None,
            },
            world.size(),
        );
        let depth_use =
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT;
        let depth = texture(gpu, WORLD, wgpu::TextureFormat::Depth32Float, depth_use);
        let depth = depth.create_view(&Default::default());
        let mut e = gpu.device.create_command_encoder(&Default::default());
        e.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.25),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        gpu.queue.submit([e.finish()]);
        let mut words = [0.0f32; FRAME_FLOATS];
        words[frame::VIEWPORT] = WORLD[0] as f32;
        words[frame::VIEWPORT + 1] = WORLD[1] as f32;
        words[frame::SCREEN] = CANVAS[0] as f32;
        words[frame::SCREEN + 1] = CANVAS[1] as f32;
        words[frame::VIGNETTE] = 0.5;
        let across = WORLD.map(|v| v.div_ceil(SIDE));
        // Every tile last changed in frame 3, except the first, which changed in frame 9.
        // Frame 9 is the frame being drawn.
        let mut changed = vec![3u32; (across[0] * across[1]) as usize];
        changed[0] = 9;
        let uniform = wgpu::BufferUsages::UNIFORM;
        Finish {
            gpu,
            world: world.create_view(&Default::default()),
            depth,
            frame: gpu.buf(&words.map(f32::to_bits), uniform),
            changed_at: gpu.buf(&changed, wgpu::BufferUsages::STORAGE),
            tiles: gpu.buf(&[0; 8], uniform),
        }
    }

    /// Writes the tiles' uniform for frame 9. The finish then draws the tiles that changed
    /// fewer than `damage_frame_cnt` frames before it, so `u32::MAX` damages every tile.
    fn damage(&self, damage_frame_cnt: u32) {
        let across = WORLD.map(|v| v.div_ceil(SIDE));
        let grow = WORLD[0] as f32 / CANVAS[0] as f32;
        let words = [
            across[0],
            across[1],
            damage_frame_cnt,
            9,
            (WORLD[0] as f32).to_bits(),
            (WORLD[1] as f32).to_bits(),
            grow.to_bits(),
            (SIDE as f32).to_bits(),
        ];
        self.gpu.queue.write_buffer(&self.tiles, 0, bytes(&words));
    }

    /// Draws the finish of `p` onto `canvases`. Each canvas keeps what it holds when `load` is
    /// set, and is cleared otherwise.
    fn draw(&self, p: &wgpu::RenderPipeline, canvases: &[&wgpu::TextureView], load: bool) {
        let g = &self.gpu.device;
        let group = g.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &p.get_bind_group_layout(0),
            entries: &[
                entry(0, self.frame.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(&self.world)),
                entry(2, wgpu::BindingResource::TextureView(&self.depth)),
                entry(30, self.changed_at.as_entire_binding()),
                entry(31, self.tiles.as_entire_binding()),
            ],
        });
        let screen_depth = texture(
            self.gpu,
            CANVAS,
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let screen_depth = screen_depth.create_view(&Default::default());
        let colors: Vec<_> = canvases.iter().map(|v| attachment(v, load)).collect();
        let mut e = g.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &colors,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &screen_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let across = WORLD.map(|v| v.div_ceil(SIDE));
            pass.set_pipeline(p);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..6, 0..across[0] * across[1]);
        }
        self.gpu.queue.submit([e.finish()]);
    }

    /// Shows `picture` whole on `canvas` (screen_show.wgsl).
    fn show(
        &self,
        p: &wgpu::RenderPipeline,
        picture: &wgpu::TextureView,
        canvas: &wgpu::TextureView,
    ) {
        let g = &self.gpu.device;
        let group = g.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &p.get_bind_group_layout(0),
            entries: &[entry(1, wgpu::BindingResource::TextureView(picture))],
        });
        let mut e = g.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[attachment(canvas, false)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(p);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.gpu.queue.submit([e.finish()]);
    }
}

fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

fn attachment(view: &wgpu::TextureView, load: bool) -> Option<wgpu::RenderPassColorAttachment<'_>> {
    Some(wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: if load {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
            },
            store: wgpu::StoreOp::Store,
        },
    })
}

fn canvas(gpu: &Gpu) -> (wgpu::Texture, wgpu::TextureView) {
    let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC;
    let t = texture(gpu, CANVAS, CANVAS_FORMAT, usage);
    let v = t.create_view(&Default::default());
    (t, v)
}

/// Reads back the texels of a canvas, one word for each texel.
fn read(gpu: &Gpu, t: &wgpu::Texture) -> Vec<u32> {
    let row = CANVAS[0] * 4;
    let out = gpu.buf(
        &vec![0; (CANVAS[0] * CANVAS[1]) as usize],
        wgpu::BufferUsages::empty(),
    );
    let mut e = gpu.device.create_command_encoder(&Default::default());
    e.copy_texture_to_buffer(
        t.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &out,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        t.size(),
    );
    gpu.queue.submit([e.finish()]);
    read_buf(&gpu.device, &gpu.queue, &out)
}

#[test]
fn the_world_s_canvas_and_picture_are_the_character_surface_s_one_canvas() {
    // A copy from a texture to a buffer takes rows in multiples of 256 bytes. `read` copies a
    // canvas's rows with no padding, so a canvas row must be such a multiple.
    const _: () = assert!((CANVAS[0] * 4).is_multiple_of(256));
    let Some(gpu) = Gpu::new() else {
        eprintln!("skipped: no adapter");
        return;
    };
    let f = Finish::new(&gpu);
    let one = pipeline(&gpu, SHADER_SCREEN_FINISH, 1, true);
    let two = pipeline(&gpu, SHADER_SCREEN_FINISH, 2, true);
    f.damage(u32::MAX);
    let (alone, alone_view) = canvas(&gpu);
    f.draw(&one, &[&alone_view], false);
    let (shown, shown_view) = canvas(&gpu);
    let (picture, picture_view) = canvas(&gpu);
    f.draw(&two, &[&shown_view, &picture_view], false);
    let alone = read(&gpu, &alone);
    assert!(
        alone.iter().any(|&w| w != alone[0]),
        "the world drawn differs pixel to pixel"
    );
    assert_eq!(
        read(&gpu, &shown),
        alone,
        "the canvas as the canvas alone holds it"
    );
    assert_eq!(read(&gpu, &picture), alone, "the picture as the canvas");

    // A frame not drawn whole shows the picture on a new canvas. It then finishes only the
    // first tile, the one changed this frame, into the canvas and the picture. Both must still
    // hold what the whole frame drew.
    let show = pipeline(&gpu, SHADER_SCREEN_SHOW, 1, false);
    let (next, next_view) = canvas(&gpu);
    f.show(&show, &picture_view, &next_view);
    f.damage(1);
    f.draw(&two, &[&next_view, &picture_view], true);
    assert_eq!(
        read(&gpu, &next),
        alone,
        "the canvas of a frame not drawn whole"
    );
    assert_eq!(read(&gpu, &picture), alone, "its picture");
}
