// pxl-render/gpu/ray/count.test.rs

use super::*;
use std::collections::BTreeSet;

const EYE: usize = count::EYE as usize;
const LIGHT: usize = count::LIGHT as usize;

/// What a frame turn does to the readback, in the renderer's order (renderer.rs `frame`):
/// the mapping out landed where its fence was stored (`landed`), refused where not `ok`;
/// the copy the frame before made asked to be mapped; each observer's queue run where it
/// is due or moved (`ran`), its owed work `owed`; the counts copied; and the frame dropped
/// with its copy where `dropped`.
#[derive(Clone, Copy, Debug)]
struct Turn {
    landed: bool,
    ok: bool,
    ran: [bool; 2],
    owed: [u32; 2],
    counted: bool,
    dropped: bool,
}

/// The readback and what the device holds beside it: each observer's owed work as its last
/// run that was not dropped kept it (`kept`, `Counter::owed`), the owed work the copy out
/// holds, and whether each observer's last run left work owed on the device (`truth`): its
/// count where it ran, owed where it was dropped (its content then cast afresh), none where it
/// was let go.
#[derive(Clone, Copy, Debug, Default)]
struct World {
    slot: Readback,
    kept: [u32; 2],
    held: [u32; 2],
    truth: [bool; 2],
    on: [bool; 2],
}

impl World {
    fn turn(&mut self, t: Turn) {
        let fence = !self.slot.mapped || t.landed;
        // A refused mapping wrote nothing: what the memory held before, none at worst.
        if self.slot.mapped && t.landed {
            self.slot.land(t.ok, if t.ok { self.held } else { [0; 2] });
        }
        self.slot.ask(fence);
        let ran = [0, 1].map(|o| t.ran[o] && self.on[o]);
        for (kept, (&ran, &owed)) in self.kept.iter_mut().zip(ran.iter().zip(&t.owed)) {
            if ran && !t.dropped {
                *kept = owed;
            }
        }
        if self.slot.copy(t.counted, ran) {
            self.held = self.kept;
        }
        for (o, &ran) in ran.iter().enumerate() {
            if ran {
                self.truth[o] = t.owed[o] != 0 || t.dropped;
            }
        }
        if t.dropped {
            self.slot.dropped();
        }
    }

    fn off(&mut self, o: usize) {
        self.slot.off(o);
        (self.truth[o], self.on[o]) = (false, false);
    }

    /// A wake is coming: a frame asked for, or a mapping whose fence wakes the turn.
    fn waking(&self) -> bool {
        self.slot.wants_frame(self.on) || self.slot.wakes(true)
    }

    /// Where an observer that may run owes work on the device, a wake is coming; the one
    /// state no step may reach is work owed and no wake.
    fn holds(&self) -> bool {
        !(0..2).any(|o| self.on[o] && self.truth[o]) || self.waking()
    }
}

/// Every turn a frame may take: each flag either way, each observer's owed work none or some.
fn turns() -> impl Iterator<Item = Turn> {
    (0..256u32).map(|b| Turn {
        landed: b & 1 != 0,
        ok: b & 2 != 0,
        ran: [b & 4 != 0, b & 8 != 0],
        owed: [(b >> 4) & 1, (b >> 5) & 1],
        counted: b & 64 != 0,
        dropped: b & 128 != 0,
    })
}

/// Every world a run of `depth` turns reaches from both observers able to run, each turn
/// any of `turns`, either observer let go or able to run again between any two.
fn reached(depth: usize) -> Vec<World> {
    let start = World {
        on: [true; 2],
        ..World::default()
    };
    let key = |w: &World| std::format!("{w:?}");
    let mut seen = BTreeSet::from([key(&start)]);
    let mut all = std::vec![start];
    let mut edge = std::vec![start];
    for _ in 0..depth {
        let mut next = Vec::new();
        for w in &edge {
            let mut grow = |a: World| {
                if seen.insert(key(&a)) {
                    next.push(a);
                }
            };
            for t in turns() {
                let mut a = *w;
                a.turn(t);
                grow(a);
            }
            for o in 0..2 {
                let mut a = *w;
                a.off(o);
                grow(a);
                let mut a = *w;
                a.on[o] = true;
                grow(a);
            }
        }
        all.extend(next.iter().copied());
        edge = next;
    }
    all
}

/// No run of turns, whatever lands, is refused, drops or is let go, reaches work owed with no
/// wake coming, for either observer: a copy skipped while a mapping is out, a refused mapping,
/// a dropped frame, the shadows toggled, the light's cast let go or made again, no world to
/// cast in, among them.
#[test]
fn owed_work_always_has_a_wake() {
    let worlds = reached(4);
    assert!(worlds.len() > 64, "{}", worlds.len());
    for w in &worlds {
        assert!(w.holds(), "owed and no wake: {w:?}");
    }
}

/// From any world reached, a scene that stands, each observer running only where it is due
/// and owing nothing more, waking only as asked, comes to rest: no wake asked and none
/// coming, and nothing owed.
#[test]
fn a_standing_scene_rests() {
    for start in reached(3) {
        let mut w = start;
        let mut rested = false;
        for _ in 0..8 {
            if !w.waking() {
                rested = true;
                break;
            }
            let due = w.slot.owed;
            w.turn(Turn {
                landed: true,
                ok: true,
                ran: due,
                owed: [0; 2],
                counted: false,
                dropped: false,
            });
        }
        assert!(rested, "no rest from {start:?}: {w:?}");
        assert!(
            !(0..2).any(|o| w.on[o] && w.truth[o]),
            "rests with work owed: {w:?}"
        );
    }
}

/// A run whose work is copied owes nothing until it lands; the frame after asks its mapping,
/// whose fence wakes the turn; a count of none rests, of some owes a run, each observer its
/// own.
#[test]
fn a_copied_count_lands_as_owed_or_rest() {
    for o in [EYE, LIGHT] {
        for (owed, then) in [(0, false), (3, true)] {
            let mut ran = [false; 2];
            ran[o] = true;
            let mut r = Readback::default();
            assert!(r.copy(false, ran));
            assert!(!r.owed[o] && r.wants_frame([true; 2]));
            assert!(r.ask(true));
            assert!(!r.wants_frame([true; 2]) && r.wakes(true) && !r.wakes(false));
            let mut read = [0; 2];
            read[o] = owed;
            assert!(!r.land(true, read));
            assert_eq!((r.owed[o], r.wants_frame([true; 2])), (then, then));
            assert!(!r.owed[1 - o]);
        }
    }
}

/// A run while a mapping is out is copied nowhere: its observer's work waits for a copy, owing
/// no run, so a frame is asked, and the mapping out, of an older generation, never settles it
/// when it lands, even as none; the next copy carries the observer's last again. The other
/// observer's reading in the same copy still counts.
#[test]
fn a_stale_generation_never_clears_a_newer_run() {
    let mut r = Readback::default();
    assert!(r.copy(false, [true, true]));
    assert!(r.ask(true));
    assert!(!r.copy(false, [true, false]));
    assert!(r.pending[EYE] && !r.owed[EYE] && r.wants_frame([true; 2]));
    r.land(true, [0, 5]);
    assert!(
        r.pending[EYE] && !r.owed[EYE],
        "an older generation settled the eye's last run: {r:?}"
    );
    assert!(
        r.owed[LIGHT],
        "the light's own last reading was dropped: {r:?}"
    );
    assert!(r.copy(false, [true, false]));
    assert!(!r.owed[EYE] && r.carried[EYE] == Some(r.cast[EYE]));
}

/// A run whose work no copy could take owes no run for it: it waits, asks for a frame, and a
/// frame that runs nothing copies it alone (`copies_pending`), which lands as owed or rest,
/// each observer its own; a frame dropped after its copy took it waits for a copy again.
#[test]
fn a_run_not_copied_is_copied_alone_and_lands() {
    for o in [EYE, LIGHT] {
        for (owed, then) in [(0, false), (3, true)] {
            let mut ran = [false; 2];
            ran[o] = true;
            let mut r = Readback::default();
            assert!(r.copy(false, [true; 2]));
            assert!(r.ask(true));
            assert!(!r.copy(false, ran));
            assert!(r.pending[o] && !r.owed[o] && !r.copies_pending());
            assert!(r.wants_frame([true; 2]));
            r.land(true, [0; 2]);
            assert!(r.copies_pending());
            assert!(r.copy(false, [false; 2]));
            r.dropped();
            assert!(r.pending[o] && !r.owed[o] && r.copies_pending());
            assert!(r.copy(false, [false; 2]));
            assert!(!r.pending[o] && r.carried[o] == Some(r.cast[o]));
            assert!(r.carried[1 - o].is_none());
            assert!(r.ask(true));
            let mut read = [0; 2];
            read[o] = owed;
            r.land(true, read);
            assert_eq!((r.owed[o], r.wants_frame([true; 2])), (then, then));
        }
    }
}

/// A refused mapping owes each observer whose work it held; one of the lanes alone owes
/// nothing.
#[test]
fn a_refused_mapping_owes_its_runs() {
    for (ran, owed) in [
        ([true, false], [true, false]),
        ([false, false], [false, false]),
    ] {
        let mut r = Readback::default();
        assert!(r.copy(true, ran));
        assert!(r.ask(true));
        assert!(!r.land(false, [0; 2]));
        assert_eq!(r.owed, owed);
    }
}

/// A dropped frame's copy owes its observers again; the next frame asks no mapping of it.
#[test]
fn a_dropped_copy_owes_its_runs() {
    let mut r = Readback::default();
    assert!(r.copy(false, [true, true]));
    r.dropped();
    assert_eq!(r.owed, [true, true]);
    assert!(!r.copied && !r.ask(true));
    assert!(r.wants_frame([true, false]) && !r.wants_frame([false, false]));
}

/// An observer let go, as the shadows go off, the light's cast is released or no world is
/// cast in: nothing it owes, no frame asked for it, and the mapping out, holding no work of
/// its next run, wakes nothing for it and owes nothing when it lands.
#[test]
fn an_observer_let_go_owes_nothing() {
    for o in [EYE, LIGHT] {
        let mut ran = [false; 2];
        ran[o] = true;
        let mut r = Readback::default();
        assert!(r.copy(false, ran));
        assert!(r.ask(true));
        r.off(o);
        assert!(!r.owed[o] && !r.wants_frame([true; 2]) && !r.wakes(true));
        r.land(true, [5; 2]);
        assert!(!r.owed[o]);
    }
}

/// The lanes alone, the `ray_counts` setting's, are copied and taken, and ask for no frame.
#[test]
fn the_lanes_alone_ask_no_frame() {
    let mut r = Readback::default();
    assert!(r.copy(true, [false; 2]));
    assert!(!r.wants_frame([true; 2]));
    assert!(r.ask(true));
    assert!(!r.wakes(true));
    assert!(r.land(true, [0; 2]));
    assert_eq!(r.owed, [false; 2]);
}
