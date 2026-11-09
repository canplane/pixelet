// pxl-render/renderer.test.rs

use super::*;
use crate::frame::shader::record::{reads_shadow_map, SHADERS};
use crate::frame::shader::record::{
    PIPELINE_DEBUG, PIPELINE_LOCATION_BEACON, PIPELINE_LOCATION_ZONE, PIPELINE_MARKER,
};
use crate::frame::shader::word::{draw, ghost_box, instance};
use crate::gpu::draw::{drifted, test_lists, Draw, DrawId, DrawList, ReceiverBox};
use crate::pass::mark::{marked, Shown, UploadOf};

/// The only test here that installs the calls' state. The lists' tests read
/// that state, so this test takes their lock (`test_lists`) and never runs
/// beside one.
#[test]
fn the_renderer_makes_every_group_as_its_layout_lays_it_out() {
    let _l = test_lists();
    let device = api::test_device();
    api::install_apart(device);
    let g = gpu().or_bug(Errno::ENODEV);
    let mut pipelines = PipelineCache::new();
    let made = (0..SHADERS).any(|_| pipelines.make_modules(g));
    assert!(
        made,
        "every module is made in as many turns as there are shaders"
    );
    // The test asserts nothing more itself: every bind group the renderer
    // makes is checked against its layout's group as it is made (api.rs
    // `Layout::holds`).
    Renderer::new(g, device, pipelines);
}

/// A draw that a scripted turn sets: the index of its id, its record's words,
/// where it is, its opacity, and whether it is a cloud's, which drifts with
/// the wind.
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

/// A scripted turn: the draws it sets, in order, the handles its lists touch,
/// and how far the wind has carried the clouds, in Q3 steps of 1/256 unit.
struct Turn {
    specs: Vec<Spec>,
    touched: Vec<u32>,
    drift: i64,
}

/// The passes that mark, as pass/*.rs make their `MarkPass`: each one's
/// pipeline, the upload a draw reads, and which of its draws are drawn.
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

/// The oracle's lists: a turn's draws rebuilt whole, with nothing kept from
/// the turns before. `records` holds every draw's record in the order it was
/// set, and a draw's uniform index is its place. A cloud's uniform holds its
/// place on the field's map, which drifts, and `drift` is the drift the turn
/// draws at.
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
        let opacity = match s.pipeline {
            PIPELINE_DECOR | PIPELINE_GHOST_DECOR | PIPELINE_IMPOSTOR => instance::OPACITY,
            _ => ghost_box::OPACITY,
        };
        let mut u = [0.0; DRAW_FLOATS];
        u[draw::Q..draw::Q + 3].copy_from_slice(&q);
        u[draw::R..draw::R + 3].copy_from_slice(&p.r);
        u[draw::DRIFTS] = drifts;
        u[opacity] = s.opacity;
        r.uniforms.push(u);
    }
    r
}

/// A mark as the passes draw it: its record without the uniform's place, its
/// uniform's bits, and whether it is the draw as it is (true) or as it was.
type Damage = ([u32; 4], [u32; DRAW_FLOATS], bool);

fn damage_of((d, u): &Draw, now: bool) -> Damage {
    ([d[0], d[1], d[2], d[4]], u.map(f32::to_bits), now)
}

/// The oracle's marks. Each pass's draws, as each of the two frames drew them
/// at its drift, are matched against the other frame's. Two draws match when
/// their records, without the uniforms' places, and their uniforms are alike
/// bit for bit. A draw with no match, or whose upload moved, marks.
fn rebuilt_marks(was: &Rebuilt, now: &Rebuilt, touched: &[u32]) -> Vec<Damage> {
    let alike = |a: &Draw, b: &Draw| damage_of(a, true) == damage_of(b, true);
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
        let changed = |d: &Draw| read(&d.0) != 0 && touched.binary_search(&read(&d.0)).is_ok();
        let drawn = |list: &[Draw], k: usize, d: &Draw| {
            list.get(k).is_some_and(|w| alike(w, d)) || list.iter().any(|w| alike(w, d))
        };
        for (k, d) in now.iter().enumerate() {
            if changed(d) || !drawn(&was, k, d) {
                out.push(damage_of(d, true));
            }
        }
        for (k, w) in was.iter().enumerate() {
            if changed(w) || !drawn(&now, k, w) {
                out.push(damage_of(w, false));
            }
        }
    }
    out.sort_unstable();
    out
}

/// The oracle's answer to whether a draw whose moves draw the frame whole
/// moved. One did if those draws' records and uniforms, in order, differ
/// between the two frames, or if an upload one reads moved.
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

/// Turn after turn, the kept draws and their changes answer as the oracle
/// does, which rebuilds the lists whole and diffs them (`Rebuilt`). The two
/// agree on whether the lists moved, whether they read the same, whether a
/// draw whose moves draw the frame whole moved, and on every pass's marks.
/// The turns draw first, draw again as they were, move a character and its
/// palette, rewrite a cloud's upload, carry the clouds on the wind, move the
/// ghost and empty the zone, let a cloud go and draw a ghost's decor, turn a
/// cloud blended, add a marker, set the markers in another order, and fill
/// the zone again.
#[test]
fn kept_draws_answer_as_the_rebuilt_lists_did() {
    let _l = test_lists();
    // In the order the core sets them (pxl-core domain/scene/stage.rs
    // `scene_step`): the ghost first, then the characters, the location, the
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
    // The ghost's decor is set right after its box, as a ghost's decor is.
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
                reads_shadow_map(s.pipeline).then(|| ReceiverBox::of(s.at, s.at.map(|v| v + 1.0)));
            let [w0, w1, w2] = s.words;
            let at = Position::of(s.at);
            match s.pipeline {
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR | PIPELINE_IMPOSTOR => list.draw_instances(
                    &ids[s.id], s.pipeline, w0, w1, w2, [0; 3], at, s.opacity, s.cloud, stand,
                ),
                _ => list.draw_in(
                    &ids[s.id], s.pipeline, w0, w1, w2, at, s.opacity, 0.0, 0.0, stand,
                ),
            }
        }
        let lists = list.take();
        let now = rebuilt(t);
        let first = k == 0;
        let uploads_changed = !t.touched.is_empty();
        let drift_changed = now.drift != was.drift;
        // The oracle's answers to what renderer.rs `frame` asks of the
        // lists.
        let records_changed = was.records != now.records;
        let bits = |r: &Rebuilt| {
            r.uniforms
                .iter()
                .map(|u| u.map(f32::to_bits))
                .collect::<Vec<_>>()
        };
        let uniforms_changed = first || bits(&was) != bits(&now);
        let rebuilt_changed = records_changed || uniforms_changed;
        let drifting = now.uniforms.iter().any(|u| drifts(u));
        let rebuilt_same = !rebuilt_changed && !uploads_changed && !(drifting && drift_changed);
        // The kept draws' answers.
        let kept_changed = first || !lists.changes.is_empty() || lists.reordered;
        let kept_same =
            !kept_changed && !uploads_changed && !(lists.draws.drifting() && drift_changed);
        assert_eq!(kept_changed, rebuilt_changed, "turn {k}: lists changed");
        assert_eq!(kept_same, rebuilt_same, "turn {k}: listed the same");
        assert_eq!(
            beside_changed(&lists, &t.touched),
            rebuilt_beside(&was, &now, &t.touched),
            "turn {k}: beside"
        );
        let m = DamageInput {
            draws: lists.draws,
            changes: lists.changes,
            touched: &t.touched,
            drift: [now.drift, was.drift],
        };
        let mut kept: Vec<Damage> = Vec::new();
        for (pipeline, read, shown) in passes() {
            marked(pipeline, (read, shown), &m, |d, is| {
                kept.push(damage_of(&d, is));
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

/// A cloud's draw drifts, and its moves draw no frame whole, whichever of
/// `BESIDE`'s pipelines draws it. The same draw without the drift does draw
/// the frame whole.
#[test]
fn a_cloud_draws_no_frame_whole() {
    let mut u = [0.0f32; DRAW_FLOATS];
    for p in BESIDE {
        let d = [1, 0, 1, 0, p];
        u[draw::DRIFTS] = 1.0;
        assert!(!beside(&d, &u), "{p}");
        u[draw::DRIFTS] = 0.0;
        assert!(beside(&d, &u), "{p}");
    }
    assert!(!beside(&[1, 0, 1, 0, PIPELINE_SKINNED], &u));
}

/// A turn as `Seen` steps it.
#[derive(Clone, Copy, Debug)]
enum Event {
    /// A frame drawn, with four flags: its lists moved, its uploads moved, the
    /// drift moved under a draw that drifts, and a draw whose moves draw the
    /// frame whole changed.
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

/// From a renderer made anew, through any order of turns, every state `Seen`
/// reaches answers as two reads would (gpu/read.rs `Read`): one over the
/// lists, and one over the draws whose moves draw the frame whole. A dropped
/// frame forgets both reads, and a turn put off forgets the second. The
/// listed draws read the same where the lists, their uploads and the drift
/// under them stood and the frame before saw them. A draw whose moves draw
/// the frame whole moved where the turn moved one or the frame before did
/// not see them.
#[test]
fn seen_answers_as_the_reads_did_in_every_state() {
    // The two reads, each as whether it holds what the last frame drew.
    let read = |fresh: [bool; 2], e: Event| -> ([bool; 2], Option<(bool, bool)>) {
        match e {
            Event::Frame([changed, uploads, drifted, beside]) => (
                [true; 2],
                Some((
                    fresh[0] && !changed && !uploads && !drifted,
                    !fresh[1] || beside,
                )),
            ),
            Event::Dropped => ([false; 2], None),
            Event::PutOff => ([fresh[0], false], None),
        }
    };
    let mut reached = vec![(Seen::NONE, [false; 2])];
    let mut k = 0;
    while k < reached.len() {
        let (s, fresh) = reached[k];
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
            let (fresh_next, want) = read(fresh, e);
            assert_eq!(got, want, "{s:?} {e:?}");
            assert_eq!([next.lists, next.beside], fresh_next, "{s:?} {e:?}");
            if !reached.iter().any(|&(r, _)| r == next) {
                reached.push((next, fresh_next));
            }
        }
        k += 1;
    }
    // Every state is reached but one, the lists unseen while their beside
    // draws are seen: a dropped frame leaves both unseen.
    assert_eq!(reached.len(), 3);
}

/// Over every combination of the reads, `FramePlan::of` decides quiet, masked
/// and unchanged as the conditions written out here do. What is unchanged is
/// also quiet, and what is masked or quiet stands.
#[test]
fn a_frame_decides_as_its_conditions_did_over_every_read() {
    for b in 0..1u32 << 8 {
        let bit = |k: u32| b >> k & 1 != 0;
        let r = FrameRead {
            partial: bit(0),
            still: bit(1),
            lists_same: bit(2),
            clouds_damage: bit(3),
            unmasked: bit(4),
            casts_px: bit(5),
            light_casts: bit(6),
        };
        let quiet = r.still && r.lists_same && !r.clouds_damage;
        let masked = r.partial && r.still && !r.unmasked;
        let unchanged = quiet && !r.casts_px && !r.light_casts;
        let d = FramePlan::of(r);
        assert_eq!(
            d,
            FramePlan {
                quiet,
                masked,
                unchanged
            },
            "{r:?}"
        );
        assert!(!d.unchanged || d.quiet, "{r:?}: what is unchanged is quiet");
        assert!(
            !(d.masked || d.quiet) || r.still,
            "{r:?}: what is masked or quiet stands"
        );
    }
}
