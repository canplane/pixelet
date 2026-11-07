// pxl-render/gpu/renderer.test.rs

use super::*;
use crate::frame::draw::{drifted, test_lists, Draw, DrawId, DrawList, Stand};
use crate::frame::shader::record::{reads_shadow_map, SHADERS};
use crate::gpu::pass::mark::{marked, Shown, UploadOf};

/// The only test here that installs the calls' state; the lists' tests read
/// it, so it takes their turn (`test_lists`).
#[test]
fn the_renderer_makes_every_group_as_its_layout_lays_it_out() {
    let _l = test_lists();
    let device = api::test_device();
    api::install_apart(device);
    let g = gpu().or_bug(Errno::ENODEV);
    let mut pipelines = Pipelines::new();
    // A module a turn at least.
    let made = (0..SHADERS).any(|_| pipelines.make_modules(g));
    assert!(
        made,
        "every module is made in as many turns as there are shaders"
    );
    // Every bind group it makes is checked against its layout's group as it is made
    // (api.rs `Layout::holds`).
    Renderer::new(g, device, pipelines);
}

/// A draw a scripted turn sets: its name's index, its record's words, where it
/// is, its opacity, and whether it is a cloud's, which drifts and which the
/// kernel bounds.
#[derive(Clone, Copy)]
struct Spec {
    id: usize,
    pipeline: u32,
    words: [u32; 3],
    at: [f64; 3],
    opacity: f32,
    cloud: bool,
}

const fn spec(id: usize, pipeline: u32, words: [u32; 3], at: [f64; 3]) -> Spec {
    Spec {
        id,
        pipeline,
        words,
        at,
        opacity: 1.0,
        cloud: false,
    }
}

const fn cloud(id: usize, pipeline: u32, words: [u32; 3], at: [f64; 3]) -> Spec {
    Spec {
        cloud: true,
        ..spec(id, pipeline, words, at)
    }
}

/// A turn: the draws it sets, in order, the handles its lists touch, and how
/// far the wind has carried the clouds, in steps of Q3.
struct Turn {
    specs: Vec<Spec>,
    touched: Vec<u32>,
    drift: i64,
}

/// The passes that mark: their pipelines, the upload a draw reads, and which
/// of their draws are drawn (gpu/pass/*.rs `Marks::new`).
fn passes() -> Vec<(u32, UploadOf, Shown)> {
    vec![
        (PIPELINE_SKINNED, |d| d[1], |_| true),
        (PIPELINE_DECOR, |d| d[0], |_| true),
        (PIPELINE_GHOST_DECOR, |d| d[0], |_| true),
        (PIPELINE_IMPOSTOR, |d| d[0], |_| true),
        (PIPELINE_LOCATION_ZONE, |d| d[0], |d| d[1] > 0),
        (PIPELINE_LOCATION_BEACON, |_| 0, |_| true),
    ]
}

/// The lists as a turn rebuilt them whole before the turns kept them: every
/// draw's record in the order it was set, its uniform's index its place, a
/// cloud's at its place on the field's map, which drifts, and the drift the
/// turn draws at.
struct Rebuilt {
    records: Vec<[u32; DRAW_WORDS]>,
    uniforms: Vec<[f32; DRAW_FLOATS]>,
    drift: Position,
}

fn rebuilt(t: &Turn) -> Rebuilt {
    let off = t.drift as f64 / 256.0;
    let mut r = Rebuilt {
        records: Vec::new(),
        uniforms: Vec::new(),
        drift: Position::of([off, off, 0.0]),
    };
    for s in &t.specs {
        let p = Position::of(s.at);
        let q = p.q.map(|v| f32::from_bits(v as u32));
        let [w0, w1, w2] = s.words;
        let drifts = if s.cloud { 1.0 } else { 0.0 };
        r.records
            .push([w0, w1, w2, r.uniforms.len() as u32, s.pipeline]);
        r.uniforms.push([
            q[0], q[1], q[2], 0.0, s.opacity, 0.0, 0.0, drifts, p.r[0], p.r[1], p.r[2], 0.0,
        ]);
    }
    r
}

/// A mark as the passes draw it: its record but its uniform's place, its
/// uniform's bits, and whether it is the draw as it is.
type Mark = ([u32; 4], [u32; DRAW_FLOATS], bool);

fn mark_of((d, u): &Draw, now: bool) -> Mark {
    ([d[0], d[1], d[2], d[4]], u.map(f32::to_bits), now)
}

/// The marks the rebuilt lists made, each pass's draws as each frame drew
/// them, at its drift, matched against the last frame's alike, their records
/// but their uniforms' places and their uniforms bit for bit, as
/// gpu/pass/mark.rs did before the turns kept them.
fn rebuilt_marks(was: &Rebuilt, now: &Rebuilt, touched: &[u32]) -> Vec<Mark> {
    let alike = |a: &Draw, b: &Draw| mark_of(a, true) == mark_of(b, true);
    let mut out = Vec::new();
    for (pipeline, read, shown) in passes() {
        let of = |r: &Rebuilt| -> Vec<Draw> {
            r.records
                .iter()
                .zip(&r.uniforms)
                .filter(|(d, _)| d[4] == pipeline && shown(d))
                .map(|(d, u)| {
                    let mut u = *u;
                    drifted(&mut u, r.drift);
                    (*d, u)
                })
                .collect()
        };
        let (was, now) = (of(was), of(now));
        let moved = |d: &Draw| read(&d.0) != 0 && touched.binary_search(&read(&d.0)).is_ok();
        let drawn = |list: &[Draw], k: usize, d: &Draw| {
            list.get(k).is_some_and(|w| alike(w, d)) || list.iter().any(|w| alike(w, d))
        };
        for (k, d) in now.iter().enumerate() {
            if moved(d) || !drawn(&was, k, d) {
                out.push(mark_of(d, true));
            }
        }
        for (k, w) in was.iter().enumerate() {
            if moved(w) || !drawn(&now, k, w) {
                out.push(mark_of(w, false));
            }
        }
    }
    out.sort_unstable();
    out
}

/// Whether the rebuilt lists moved a draw whose moves draw the frame whole,
/// as gpu/renderer.rs read them before the turns kept them: those draws'
/// records and uniforms in order, and whether an upload one reads moved.
fn rebuilt_beside(was: &Rebuilt, now: &Rebuilt, touched: &[u32]) -> bool {
    let beside = |r: &Rebuilt| -> Vec<([u32; DRAW_WORDS], [u32; DRAW_FLOATS])> {
        r.records
            .iter()
            .zip(&r.uniforms)
            .filter(|(d, u)| beside(d, &u[..]))
            .map(|(d, u)| (*d, u.map(f32::to_bits)))
            .collect()
    };
    let (w, n) = (beside(was), beside(now));
    w != n || n.iter().any(|(d, _)| touched.binary_search(&d[0]).is_ok())
}

/// What the kept draws and their changes answer equals what the rebuilt lists
/// and the diffs over them answered, a turn after another: whether the lists
/// moved, whether they read the same, whether a draw whose moves draw the
/// frame whole moved, and every pass's marks. The turns draw first, draw
/// again as they were, move a character and its palette, rewrite a cloud's upload, carry the clouds
/// on the wind, move the ghost and empty the zone, let a cloud go and draw a
/// ghost's decor, turn a cloud blended, add a marker and set the markers in
/// another order, and fill the zone again.
#[test]
fn kept_draws_answer_as_the_rebuilt_lists_did() {
    let _l = test_lists();
    // In the order the kernel sets them (pxl-kernel domain/scene/stage.rs
    // `visible_step`): the ghost first, then the characters, the location, the
    // gizmos, the clouds and the markers.
    let base = vec![
        spec(4, PIPELINE_GHOST_BOX, [40, 1, 0], [6.0, 6.0, 6.0]),
        spec(0, PIPELINE_SKINNED, [10, 11, 1], [1.0, 2.0, 3.0]),
        spec(1, PIPELINE_LOCATION_ZONE, [20, 4, 0], [4.0, 4.0, 0.0]),
        spec(2, PIPELINE_LOCATION_BEACON, [0, 0, 0], [5.0, 5.0, 5.0]),
        spec(8, PIPELINE_DEBUG, [60, 2, 0], [7.0, 7.0, 7.0]),
        cloud(5, PIPELINE_DECOR, [50, 0, 3], [256.0, 512.0, 0.0]),
        cloud(6, PIPELINE_IMPOSTOR, [50, 0, 2], [256.0, 512.0, 0.0]),
        cloud(7, PIPELINE_DECOR, [51, 1, 4], [512.0, 512.0, 0.0]),
        spec(3, PIPELINE_MARKER, [30, 2, 0], [0.0; 3]),
    ];
    let with = |edit: &dyn Fn(&mut Vec<Spec>)| {
        let mut s = base.clone();
        edit(&mut s);
        s
    };
    let turn = |specs: Vec<Spec>, touched: &[u32], drift: i64| Turn {
        specs,
        touched: touched.to_vec(),
        drift,
    };
    let character_moved = |s: &mut Vec<Spec>| s[1].at = [1.5, 2.0, 3.0];
    let ghost_moved = |s: &mut Vec<Spec>| {
        character_moved(s);
        s[0].at = [6.0, 7.0, 6.0];
        s[2].words[1] = 0;
    };
    // The ghost's decor set right after its box, as one ghost's would be.
    let cloud_gone = |s: &mut Vec<Spec>| {
        ghost_moved(s);
        s.remove(7);
        s.insert(
            1,
            spec(9, PIPELINE_GHOST_DECOR, [41, 2, 1], [8.0, 8.0, 8.0]),
        );
    };
    let blended = |s: &mut Vec<Spec>| {
        cloud_gone(s);
        s[6].pipeline = PIPELINE_GHOST_DECOR;
        s[6].opacity = 0.5;
    };
    let marked_twice = |s: &mut Vec<Spec>| {
        blended(s);
        s.push(spec(10, PIPELINE_MARKER, [31, 1, 0], [0.0; 3]));
    };
    let reordered = |s: &mut Vec<Spec>| {
        marked_twice(s);
        let m = s.pop().unwrap();
        let at = s.len() - 1;
        s.insert(at, m);
    };
    let filled = |s: &mut Vec<Spec>| {
        reordered(s);
        s[3].words[1] = 4;
    };
    let turns = [
        turn(base.clone(), &[10, 11, 20, 30, 40, 50, 51, 60], 0),
        turn(base.clone(), &[], 0),
        turn(with(&character_moved), &[11], 0),
        turn(with(&character_moved), &[50], 0),
        turn(with(&character_moved), &[], 3),
        turn(with(&ghost_moved), &[], 3),
        turn(with(&cloud_gone), &[41, 51], 3),
        turn(with(&blended), &[], 3),
        turn(with(&marked_twice), &[31], 3),
        turn(with(&reordered), &[], 3),
        turn(with(&filled), &[], 3),
    ];
    let ids: Vec<DrawId> = (0..11).map(|_| DrawId::default()).collect();
    let mut list = DrawList::default();
    let mut was = Rebuilt {
        records: Vec::new(),
        uniforms: Vec::new(),
        drift: Position::of([0.0; 3]),
    };

    for (k, t) in turns.iter().enumerate() {
        for s in &t.specs {
            let stand =
                reads_shadow_map(s.pipeline).then(|| Stand::of(s.at, s.at.map(|v| v + 1.0)));
            let [w0, w1, w2] = s.words;
            let at = Position::of(s.at);
            match s.pipeline {
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR | PIPELINE_IMPOSTOR => list.draw_instances(
                    &ids[s.id], s.pipeline, w0, w1, w2, [0; 3], at, s.opacity, s.cloud, stand,
                ),
                _ => list.draw_as(
                    &ids[s.id], s.pipeline, w0, w1, w2, at, 0.0, s.opacity, stand,
                ),
            }
        }
        let lists = list.take();
        let now = rebuilt(t);
        let first = k == 0;
        let uploads_moved = !t.touched.is_empty();
        let drift_moved = now.drift != was.drift;
        // The rebuilt lists' answers (gpu/renderer.rs `frame`).
        let records_moved = was.records != now.records;
        let bits = |r: &Rebuilt| {
            r.uniforms
                .iter()
                .map(|u| u.map(f32::to_bits))
                .collect::<Vec<_>>()
        };
        let uniforms_moved = first || bits(&was) != bits(&now);
        let rebuilt_moved = records_moved || uniforms_moved;
        let drifting = now.uniforms.iter().any(|u| drifts(u));
        let rebuilt_same = !rebuilt_moved && !uploads_moved && !(drifting && drift_moved);
        // The kept draws' answers.
        let kept_moved = first || !lists.changes.is_empty() || lists.reordered;
        let kept_same = !kept_moved && !uploads_moved && !(lists.draws.drifting() && drift_moved);
        assert_eq!(kept_moved, rebuilt_moved, "turn {k}: lists moved");
        assert_eq!(kept_same, rebuilt_same, "turn {k}: listed the same");
        assert_eq!(
            beside_changed(&lists, &t.touched),
            rebuilt_beside(&was, &now, &t.touched),
            "turn {k}: beside"
        );
        let m = MarkInput {
            draws: lists.draws,
            changes: lists.changes,
            touched: &t.touched,
            drift: [now.drift, was.drift],
        };
        let mut kept: Vec<Mark> = Vec::new();
        for (pipeline, read, shown) in passes() {
            marked(&[pipeline], (read, shown), &m, |d, is| {
                kept.push(mark_of(&d, is));
            });
        }
        kept.sort_unstable();
        assert_eq!(
            kept,
            rebuilt_marks(&was, &now, &t.touched),
            "turn {k}: marks"
        );
        was = now;
    }
}

/// A cloud's draws, which drift, move no frame whole, whichever of decor's
/// pipelines draws them; the ghost's, which drift not, do.
#[test]
fn a_cloud_draws_no_frame_whole() {
    let mut u = [0.0f32; DRAW_FLOATS];
    for p in BESIDE {
        let d = [1, 0, 1, 0, p];
        u[7] = 1.0;
        assert!(!beside(&d, &u), "{p}");
        u[7] = 0.0;
        assert!(beside(&d, &u), "{p}");
    }
    assert!(!beside(&[1, 0, 1, 0, PIPELINE_SKINNED], &u));
}

/// A turn as `Seen` steps it.
#[derive(Clone, Copy, Debug)]
enum Event {
    /// A frame drawn: its lists moved, its uploads moved, the drift moved
    /// under a draw that drifts, and a draw whose moves draw the frame whole
    /// changed.
    Frame([bool; 4]),
    Dropped,
    PutOff,
}

fn events() -> Vec<Event> {
    let mut e: Vec<Event> = (0..16)
        .map(|b| Event::Frame([0, 1, 2, 3].map(|k| b >> k & 1 != 0)))
        .collect();
    e.extend([Event::Dropped, Event::PutOff]);
    e
}

/// Every state `Seen` reaches from a renderer made anew, each turn after any
/// other, answers as the rebuilt lists' reads did (gpu/read.rs `Read`, each
/// forgotten where the frame was dropped, and the beside's where a turn was
/// put off too): the listed draws read the same where the lists, their
/// uploads and the drift under them stood and the frame before saw them, and a
/// draw whose moves draw the frame whole moved where the turn moved one or
/// the frame before did not see them.
#[test]
fn seen_answers_as_the_reads_did_in_every_state() {
    // The rebuilt lists' reads, whether each holds what the last frame drew.
    let read = |held: [bool; 2], e: Event| -> ([bool; 2], Option<(bool, bool)>) {
        match e {
            Event::Frame([moved, uploads, drifted, beside]) => (
                [true; 2],
                Some((
                    held[0] && !moved && !uploads && !drifted,
                    !held[1] || beside,
                )),
            ),
            Event::Dropped => ([false; 2], None),
            Event::PutOff => ([held[0], false], None),
        }
    };
    let mut reached = vec![(Seen::NONE, [false; 2])];
    let mut k = 0;
    while k < reached.len() {
        let (s, held) = reached[k];
        for e in events() {
            let mut next = s;
            let got = match e {
                Event::Frame([m, u, d, b]) => Some(next.frame(m, u, d, b)),
                Event::Dropped => {
                    next.dropped();
                    None
                }
                Event::PutOff => {
                    next.put_off();
                    None
                }
            };
            let (held_next, want) = read(held, e);
            assert_eq!(got, want, "{s:?} {e:?}");
            assert_eq!([next.lists, next.beside], held_next, "{s:?} {e:?}");
            if !reached.iter().any(|&(r, _)| r == next) {
                reached.push((next, held_next));
            }
        }
        k += 1;
    }
    // All but the lists unseen and their beside draws seen: a frame dropped
    // leaves both unseen.
    assert_eq!(reached.len(), 3);
}

/// Every frame's reads decide what it draws as the renderer's conditions did
/// before they were one function (gpu/renderer.rs `frame`): quiet, gated and
/// stood, over every one of the reads.
#[test]
fn a_frame_decides_as_its_conditions_did_over_every_read() {
    for b in 0..1u32 << 8 {
        let bit = |k: u32| b >> k & 1 != 0;
        let r = Reads {
            partial: bit(0),
            stands: bit(1),
            listed_same: bit(2),
            clouds_drawn: bit(3),
            ungated: bit(4),
            casts_px: bit(5),
            light_casts: bit(6),
        };
        let quiet = r.stands && r.listed_same && !r.clouds_drawn;
        let gated = r.partial && r.stands && !r.ungated;
        let stood = quiet && !r.casts_px && !r.light_casts;
        let d = Decided::of(r);
        assert_eq!(
            d,
            Decided {
                quiet,
                gated,
                stood
            },
            "{r:?}"
        );
        assert!(!d.stood || d.quiet, "{r:?}: what stands is quiet");
        assert!(
            !(d.gated || d.quiet) || r.stands,
            "{r:?}: what is gated or quiet stands"
        );
    }
}
