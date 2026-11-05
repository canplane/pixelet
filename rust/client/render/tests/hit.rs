//! The missed pixels' hits by hand answer as `lead` and `faces_put` here do: `hit_lead_cs`
//! (frame/shader/hit_lead.wgsl) leads each square's patch of hits alike and lists its ask,
//! `hit_faces_cs` (frame/shader/hit_faces.wgsl) puts the faces asked at every hit's pixel.
//! The hits are as the walks again keep them (miss_hit.wgsl `keep_hit`): a pixel waits for
//! its hit's number, hits taken in no order, patches of one node and face across squares'
//! edges, neighbors alike but for the face or a root, stale marks, pixels at the screen's
//! edges, counts past a workgroup and past `beam::HITS`, a lone hit, an earlier cast's hits
//! past the count. Every word of the beam rays', the walk again's and the rays' compared,
//! the asks as a set, as the order their places are taken in is the device's. Skipped where
//! no adapter answers.

mod gpu;

use pxl_engine::beam::{
    Grid, ASK_FACE, ASK_LINE, ASK_NODE, ASK_TO, ASK_WORDS, CELL_CORNER, HITS, HIT_FACE,
    HIT_FOLLOWS, HIT_LANES, HIT_NODE, HIT_OUT, HIT_PX, HIT_SQUARE, HIT_WAITS,
};
use pxl_engine::{eye, node_cache, raster, ray};
use pxl_render::frame::shader::record::{src, SHADER_HIT_FACES, SHADER_HIT_LEAD};

use gpu::{Gpu, Tally};

fn mix(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ h >> 12
}

/// A screen the hits are kept on: its pixels, its tangents, the eye's jitter, how many of
/// its pixels hit, and how many the walks counted.
struct Screen {
    name: &'static str,
    size: [u32; 2],
    tan: [f32; 2],
    jitter: [f32; 2],
    /// Of 16, the pixels that keep a hit.
    dense: u32,
    /// The count past the hits kept, as the walks' count runs on past `HITS`.
    over: u32,
    /// The hits kept at most.
    most: u32,
}

const SCREENS: [Screen; 5] = [
    Screen {
        name: "a phone's, few hits",
        size: [333, 197],
        tan: [0.36, 0.64],
        jitter: [0.25, -0.375],
        dense: 5,
        over: 0,
        most: HITS,
    },
    Screen {
        name: "a wide one, most hits",
        size: [640, 256],
        tan: [2.4, 0.9],
        jitter: [-0.5, 0.5],
        dense: 15,
        over: 0,
        most: HITS,
    },
    Screen {
        name: "a 16:9 one at 90°, half its pixels",
        size: [480, 270],
        tan: [1.0, 0.5625],
        jitter: [0.125, 0.0625],
        dense: 8,
        over: 0,
        most: HITS,
    },
    Screen {
        name: "every pixel a hit, past `HITS`",
        size: [1024, 512],
        tan: [1.0, 0.5],
        jitter: [0.0, 0.0],
        dense: 16,
        over: 77,
        most: HITS,
    },
    Screen {
        name: "one hit, so one ask opens the faces' one workgroup",
        size: [96, 64],
        tan: [1.0, 0.75],
        jitter: [0.375, -0.125],
        dense: 8,
        over: 0,
        most: 1,
    },
];

/// The ulps of a cell's quotient from a whole number within which the pixel's ray may lie
/// in the next cell: the GPU's own division of the pixel's tangent (grid.wgsl `px_tan`) and
/// the sum it may fuse; the quotient itself is a product by the host's `Grid::s_inverse`.
const EDGE_ULPS: f64 = 8.0;

/// The patch pixel `p` of `seed`'s screen lies in: rectangles of a few to tens of pixels,
/// laid across squares' edges.
fn patch(p: [u32; 2], seed: u32) -> u32 {
    let (bx, by) = (p[0] / (3 + seed % 23), p[1] / (2 + seed % 17));
    mix(bx, by ^ seed)
}

/// The node and face a hit of `patch` stops at, in its words to `HIT_PX`: a node's zoom,
/// corner and roots, and its face; one patch in four alike its neighbor's but for the face
/// or a root.
fn stop(patch: u32) -> [u32; HIT_PX as usize] {
    let mut w = [0u32; HIT_PX as usize];
    let base = patch & !3;
    for (k, v) in w.iter_mut().enumerate() {
        *v = mix(base, k as u32);
    }
    match patch % 4 {
        1 => w[HIT_FACE as usize] ^= 1,
        2 => w[(HIT_FACE - 1) as usize] ^= 1,
        _ => {}
    }
    w
}

/// The hits a screen keeps and the words they lie in: the beam rays' (`keep_hit`'s), the
/// rays' last word a pixel, and the walks' count.
struct Kept {
    beams: Vec<u32>,
    faces: Vec<u32>,
    cnt: u32,
}

fn kept(g: &Grid, s: &Screen, seed: u32) -> Kept {
    let [w, h] = s.size;
    let mut beams: Vec<u32> = (0..g.size()).map(|k| mix(k, seed ^ 0x55)).collect();
    // The cells' starts' corners, each its own.
    for y in 0..g.span[1] {
        for x in 0..g.span[0] {
            beams[(g.cell_at([x, y]) + CELL_CORNER) as usize] = y * g.corners[0] + x;
        }
    }
    let mut faces: Vec<u32> = (0..w * h).map(|k| mix(k, seed) & !HIT_WAITS).collect();
    // The hit pixels, numbered in an order of their own, as the walks' atomics take them.
    let mut hit: Vec<u32> = (0..w * h)
        .filter(|&k| mix(k, seed ^ 7) % 16 < s.dense)
        .collect();
    hit.sort_by_key(|&k| mix(k, seed ^ 9));
    hit.truncate(s.most.min(HITS) as usize);
    for (r, &k) in hit.iter().enumerate() {
        let p = [k % w, k / w];
        let at = g.hit_at(r as u32) as usize;
        beams[at..at + HIT_PX as usize].copy_from_slice(&stop(patch(p, seed)));
        beams[at + HIT_PX as usize] = p[1] << 16 | p[0];
        faces[k as usize] = HIT_WAITS | r as u32;
    }
    // Past the hits kept, a workgroup's worth of an earlier cast's: pixels on the screen, faces
    // their own, which no thread past the count may put.
    for r in hit.len() as u32..(hit.len() as u32 + HIT_LANES).min(HITS) {
        let at = g.hit_at(r) as usize;
        let k = mix(r, seed ^ 11) % (w * h);
        beams[at + HIT_PX as usize] = ((k / w) << 16) | (k % w);
        beams[at + HIT_OUT as usize] = mix(r, seed ^ 13) & !HIT_FOLLOWS;
    }
    // Stale marks: a waiting number past `HITS`, or one whose hit is another pixel's.
    for k in 0..w * h {
        let m = mix(k, seed ^ 3) % 64;
        if faces[k as usize] & HIT_WAITS == 0 && m == 0 {
            faces[k as usize] = HIT_WAITS | (HITS + m);
        } else if faces[k as usize] & HIT_WAITS == 0 && m == 1 && !hit.is_empty() {
            faces[k as usize] = HIT_WAITS | (mix(k, 1) % hit.len() as u32);
        }
    }
    Kept {
        beams,
        faces,
        cnt: hit.len() as u32 + s.over,
    }
}

/// The eye's words for a screen: its tangents, its jitter and its grid's.
fn eye_words(g: &Grid, s: &Screen) -> Vec<u32> {
    let mut e = vec![0u32; eye::WORDS];
    e[eye::TAN] = s.tan[0].to_bits();
    e[eye::TAN + 1] = s.tan[1].to_bits();
    e[eye::JITTER] = s.jitter[0].to_bits();
    e[eye::JITTER + 1] = s.jitter[1].to_bits();
    e[eye::GRID..eye::GRID + eye::GRID_WORDS].copy_from_slice(&g.words());
    e
}

/// The tangents of pixel `p`'s ray, as grid.wgsl `px_tan` takes them.
fn px_tan(s: &Screen, p: [u32; 2]) -> [f32; 2] {
    let ndc_x = (p[0] as f32 + 0.5 + s.jitter[0]) / s.size[0] as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p[1] as f32 + 0.5 + s.jitter[1]) / s.size[1] as f32 * 2.0;
    [ndc_x * s.tan[0], ndc_y * s.tan[1]]
}

/// The cells pixel `p`'s ray may lie in by a GPU's rounding: `Grid::cell`'s, and where its
/// quotient falls within `EDGE_ULPS` of a whole number, the one on its other side.
fn cells_of(g: &Grid, s: &Screen, p: [u32; 2]) -> [[u32; 2]; 2] {
    let cell = g.cell(px_tan(s, p));
    let mut other = cell;
    for a in 0..2 {
        let ndc =
            (f64::from(p[a]) + 0.5 + f64::from(s.jitter[a])) / f64::from(s.size[a]) * 2.0 - 1.0;
        let t = f64::from(s.tan[a]);
        // Across, `tx + tan`; up, `tan - ty`, `ty` the negated `ndc`'s: alike.
        let v = (ndc * t + t) * f64::from(g.s_inverse());
        let ulp = {
            let x = v.abs() as f32;
            f64::from(f32::from_bits(x.to_bits() + 1) - x)
        };
        if (v - v.round_ties_even()).abs() <= EDGE_ULPS * ulp {
            let n = v.round_ties_even() as u32;
            other[a] = if cell[a] < n { n } else { n.saturating_sub(1) }.min(g.span[a] - 1);
        }
    }
    [cell, other]
}

/// What `hit_lead_cs` writes, thread by thread in the order of the hits: the beam rays' and
/// the walk again's words.
fn lead(g: &Grid, s: &Screen, kept: &Kept, beams: &mut [u32], marks: &mut [u32]) {
    let w = s.size[0];
    let alike_hit = |beams: &[u32], k: u32, x: u32, y: u32| {
        let f = kept.faces[(y * w + x) as usize];
        if f & HIT_WAITS == 0 {
            return HITS;
        }
        let r = f & !HIT_WAITS;
        let a = g.hit_at(r);
        if r >= HITS || (0..HIT_PX).any(|j| beams[(a + j) as usize] != beams[(k + j) as usize]) {
            return HITS;
        }
        r
    };
    for id in 0..kept.cnt.min(HITS) {
        let k = g.hit_at(id);
        let px = beams[(k + HIT_PX) as usize];
        let (x, y) = (px & 0xffff, px >> 16);
        let mut from = HITS;
        if x % HIT_SQUARE != 0 {
            from = alike_hit(beams, k, x - 1, y);
        }
        if from == HITS && y % HIT_SQUARE != 0 {
            from = alike_hit(beams, k, x, y - 1);
        }
        if from != HITS {
            beams[(k + HIT_OUT) as usize] = HIT_FOLLOWS | from;
            continue;
        }
        let asked = ray::MISS_ARGS_ROOM as usize;
        let place = marks[asked + raster::ARGS_INSTANCES as usize];
        marks[asked + raster::ARGS_INSTANCES as usize] += 1;
        if place.is_multiple_of(raster::FACES_LANES) {
            let f = &mut marks[asked + raster::ARGS_FACES as usize];
            *f = (*f).max(place / raster::FACES_LANES + 1);
        }
        let cell = g.cell(px_tan(s, [x, y]));
        let corner = beams[(g.cell_at(cell) + CELL_CORNER) as usize];
        let a = g.hit_ask_at(place) as usize;
        beams[a + ASK_NODE as usize] = k + HIT_NODE;
        beams[a + ASK_FACE as usize] = beams[(k + HIT_FACE) as usize];
        beams[a + ASK_LINE as usize] = Grid::line(corner);
        beams[a + ASK_TO as usize] = k + HIT_OUT;
    }
}

/// The asks `n` leads listed, each its words, in the order of the hit each is for, so the
/// device's order of places is none of the comparison's.
fn sorted_asks(g: &Grid, beams: &mut [u32], n: u32) {
    let at = g.hit_ask_at(0) as usize;
    let mut asks: Vec<[u32; ASK_WORDS as usize]> = beams[at..at + (n * ASK_WORDS) as usize]
        .chunks(ASK_WORDS as usize)
        .map(|c| c.try_into().unwrap())
        .collect();
    asks.sort_by_key(|a| a[ASK_TO as usize]);
    beams[at..at + (n * ASK_WORDS) as usize].copy_from_slice(asks.concat().as_slice());
}

/// What `hit_faces_cs` writes: each hit's faces at its
/// pixel, followed to the hit that asked them.
fn faces_put(g: &Grid, s: &Screen, kept: &Kept, beams: &[u32], faces: &mut [u32]) {
    for id in 0..kept.cnt.min(HITS) {
        let k = g.hit_at(id);
        let px = beams[(k + HIT_PX) as usize];
        let mut f = beams[(k + HIT_OUT) as usize];
        let mut steps = 0;
        while f & HIT_FOLLOWS != 0 && steps < 2 * HIT_SQUARE {
            f = beams[(g.hit_at(f & !HIT_FOLLOWS) + HIT_OUT) as usize];
            steps += 1;
        }
        faces[((px >> 16) * s.size[0] + (px & 0xffff)) as usize] = f;
    }
}

/// The rays' texture of `size`, its last word a pixel `faces` and the rest of a pattern;
/// read back whole, layer by layer.
fn rays(gpu: &Gpu, size: [u32; 2], faces: &[u32]) -> (wgpu::Texture, Vec<u32>) {
    let texels = (size[0] * size[1]) as usize;
    let mut words: Vec<u32> = (0..ray::WORDS as usize * texels)
        .map(|k| mix(k as u32, 0xab))
        .collect();
    words[ray::FACES as usize * texels..].copy_from_slice(faces);
    let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: ray::WORDS,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        t.as_image_copy(),
        gpu::bytes(&words),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * 4),
            rows_per_image: Some(size[1]),
        },
        t.size(),
    );
    (t, words)
}

fn read_rays(gpu: &Gpu, t: &wgpu::Texture) -> Vec<u32> {
    let [w, h, layers] = [t.width(), t.height(), t.depth_or_array_layers()];
    let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let staging = gpu.buffer(
        &vec![0u32; (row / 4 * h * layers) as usize],
        wgpu::BufferUsages::empty(),
    );
    let mut e = gpu.device.create_command_encoder(&Default::default());
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
    gpu.queue.submit([e.finish()]);
    let padded = gpu.read(&staging);
    padded
        .chunks((row / 4) as usize)
        .flat_map(|r| r[..w as usize].iter().copied())
        .collect()
}

#[test]
fn hand_hits_answer_as_hit_lead_and_faces() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let lead_p = gpu.pipeline(src(SHADER_HIT_LEAD), "hit_lead_cs");
    let faces_p = gpu.pipeline(src(SHADER_HIT_FACES), "hit_faces_cs");
    let store = wgpu::BufferUsages::STORAGE;
    let mut tally = Tally::default();
    let (mut leads, mut follows, mut edges, mut edges_apart) = (0u32, 0u32, 0u32, 0u32);
    for (n, s) in SCREENS.iter().enumerate() {
        let seed = 0x1000 + n as u32;
        let g = Grid::of(s.tan, 1);
        let k = kept(&g, s, seed);
        let hits = k.cnt.min(HITS);
        // A workgroup more than the hits fill, as no dispatch is short of them.
        let groups = [hits.div_ceil(HIT_LANES) + 1, 1, 1];
        let eye_buf = gpu.buffer(&eye_words(&g, s), wgpu::BufferUsages::UNIFORM);
        let mut head = vec![0u32; (node_cache::HITS_CNT + 1) as usize];
        head[node_cache::HITS_CNT as usize] = k.cnt;
        let node_cache_buf = gpu.buffer(&head, store);
        let mut marks = vec![0u32; (ray::MISS_ARGS_ROOM + raster::ARGS_WORDS) as usize];
        marks[(ray::MISS_ARGS_ROOM + raster::ARGS_FACES + 1) as usize] = 1;
        marks[(ray::MISS_ARGS_ROOM + raster::ARGS_FACES + 2) as usize] = 1;

        // The leads.
        let (texture, ray_words) = rays(&gpu, s.size, &k.faces);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let beams_buf = gpu.buffer(&k.beams, store);
        let marks_buf = gpu.buffer(&marks, store);
        gpu.dispatch(
            &lead_p,
            &[
                (0, eye_buf.as_entire_binding()),
                (3, wgpu::BindingResource::TextureView(&view)),
                (8, beams_buf.as_entire_binding()),
                (9, node_cache_buf.as_entire_binding()),
                (14, marks_buf.as_entire_binding()),
            ],
            groups,
        );
        let mut want = k.beams.clone();
        lead(&g, s, &k, &mut want, &mut marks);
        let asks = marks[(ray::MISS_ARGS_ROOM + raster::ARGS_INSTANCES) as usize];
        let mut have = gpu.read(&beams_buf);
        let have_marks = gpu.read(&marks_buf);
        let lead_beams = want.clone();
        sorted_asks(&g, &mut want, asks);
        sorted_asks(
            &g,
            &mut have,
            asks.min(have_marks[(ray::MISS_ARGS_ROOM + raster::ARGS_INSTANCES) as usize]),
        );
        // An ask whose pixel's ray lies on a cell's edge reads either cell's line.
        let mut either = vec![None; want.len()];
        for id in 0..hits {
            let at = g.hit_at(id);
            let px = k.beams[(at + HIT_PX) as usize];
            let p = [px & 0xffff, px >> 16];
            let [cell, other] = cells_of(&g, s, p);
            if cell != other {
                edges += 1;
                let line = |c: [u32; 2]| Grid::line(k.beams[(g.cell_at(c) + CELL_CORNER) as usize]);
                // The ask for this hit lies where the sorted asks put it.
                let sorted_at =
                    (0..asks).find(|&q| want[(g.hit_ask_at(q) + ASK_TO) as usize] == at + HIT_OUT);
                if let Some(q) = sorted_at {
                    let w = (g.hit_ask_at(q) + ASK_LINE) as usize;
                    edges_apart += u32::from(have[w] != want[w]);
                    either[w] = Some(line(other));
                }
            }
        }
        for id in 0..hits {
            let out = want[(g.hit_at(id) + HIT_OUT) as usize];
            follows += u32::from(
                out & HIT_FOLLOWS != 0 && out != k.beams[(g.hit_at(id) + HIT_OUT) as usize],
            );
        }
        leads += asks;
        tally.compare_by(
            &format!("{}: leads' beam rays' words", s.name),
            &want,
            &have,
            |w| format!("word {w}"),
            |w, a, b| a == b || either[w] == Some(b),
        );
        tally.compare(
            &format!("{}: leads' args", s.name),
            &marks,
            &have_marks,
            |w| format!("args word {w}"),
        );
        tally.compare(
            &format!("{}: leads' rays", s.name),
            &ray_words,
            &read_rays(&gpu, &texture),
            |w| format!("ray word {w}"),
        );

        // The faces asked, as the faces' pass puts them where each ask says, then put at
        // the pixels.
        let mut asked = lead_beams.clone();
        for q in 0..asks {
            let to = asked[(g.hit_ask_at(q) + ASK_TO) as usize];
            asked[to as usize] = mix(to, seed) & !HIT_FOLLOWS;
        }
        let (texture, mut ray_words) = rays(&gpu, s.size, &k.faces);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let asked_buf = gpu.buffer(&asked, store);
        gpu.dispatch(
            &faces_p,
            &[
                (0, eye_buf.as_entire_binding()),
                (3, wgpu::BindingResource::TextureView(&view)),
                (8, asked_buf.as_entire_binding()),
                (9, node_cache_buf.as_entire_binding()),
            ],
            groups,
        );
        let texels = (s.size[0] * s.size[1]) as usize;
        faces_put(
            &g,
            s,
            &k,
            &asked,
            &mut ray_words[ray::FACES as usize * texels..],
        );
        tally.compare(
            &format!("{}: faces' rays", s.name),
            &ray_words,
            &read_rays(&gpu, &texture),
            |w| format!("layer {} pixel {}", w / texels, w % texels),
        );
        tally.compare(
            &format!("{}: faces' beam rays' words", s.name),
            &asked,
            &gpu.read(&asked_buf),
            |w| format!("word {w}"),
        );
    }
    eprintln!("{leads} hits lead, {follows} follow, {edges} pixels' rays on a cell's edge, {edges_apart} of them the other cell's");
    assert!(leads > 0 && follows > 0, "hits both lead and follow");
    tally.end("hits");
}
