// pxl-render/gpu/ray/schedule.test.rs

use super::super::count::Readback;
use super::*;
use fnv::FnvHashSet;
use pxl_engine::screen::JITTERS;

const E: usize = EYE as usize;
const L: usize = LIGHT as usize;

/// A drawn pixel's parked state, of a region of them: no parked ray, a ray of the rays now
/// cast parked, or a ray of rays no longer cast (the view, the world, the jitter or the surface
/// moved since it was parked).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Park {
    No,
    Now,
    Gone,
}

/// The pixels as two regions: the tiles a gated cast takes (`CLAIMED`), and the rest, which
/// only a cast drawn whole takes.
const CLAIMED: usize = 0;
const REGIONS: usize = 2;

/// What a frame turn reads and what befalls it, in the renderer's order (renderer.rs
/// `frame`): the mapping out landed, refused or not (`landed`: none, ok, refused); a world to
/// cast in or none (`present`); what the schedule reads (`Reads`), the observers' owed
/// work the readback's or, where `owed` names it, that and no readback stepped; whether the walks leave work parked, the eye's and the light's
/// (`parks`, `light_owed`); whether the frame is drawn whole after all (`whole`, renderer.rs
/// `Decided::gated` not); and whether it is dropped.
#[derive(Clone, Copy, Debug)]
struct Turn {
    landed: u8,
    present: bool,
    same: bool,
    jitter: u32,
    gated: bool,
    shadows: bool,
    made: bool,
    content_same: bool,
    world_columns: bool,
    corner: i32,
    receivers_same: bool,
    forgets: bool,
    owed: Option<[bool; 2]>,
    parks: bool,
    light_owed: bool,
    whole: bool,
    dropped: bool,
}

/// The schedule, the readback, and what the device holds beside them: each observer's owed
/// work as its last run that was not dropped kept it, the owed work the copy out holds, and
/// whether each observer's last run left work owed (`truth`), as count.test.rs's `World`;
/// each region's parked state; the regions each of the eye's two owed lists names; the owed
/// list the last eye run that was not dropped filled; the jitter the rays were last cast
/// at, the view's own; and whether the node cache forgot anything since the light last cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct World {
    s: Schedule,
    slot: Readback,
    kept: [u32; 2],
    held: [u32; 2],
    truth: [bool; 2],
    parked: [Park; REGIONS],
    lists: [[bool; REGIONS]; 2],
    filled: Option<u32>,
    jitter: u32,
    forgot: bool,
}

impl World {
    fn new() -> World {
        World {
            s: Schedule::new(),
            slot: Readback::default(),
            kept: [0; 2],
            held: [0; 2],
            truth: [false; 2],
            parked: [Park::No; REGIONS],
            lists: [[false; REGIONS]; 2],
            filled: None,
            jitter: 0,
            forgot: false,
        }
    }

    /// The rays the parked states were parked by are no longer cast.
    fn rays_moved(&mut self) {
        for p in &mut self.parked {
            if *p == Park::Now {
                *p = Park::Gone;
            }
        }
    }

    /// One turn; `Err` names the invariant a step broke.
    fn turn(&mut self, t: Turn) -> Result<Plan, &'static str> {
        // The counts the frames before copied, read back before the cast is decided
        // (renderer.rs `counts_back`).
        let joined = t.owed.is_none();
        if joined {
            let fence = !self.slot.mapped || t.landed != 0;
            if self.slot.mapped && t.landed != 0 {
                let ok = t.landed == 1;
                self.slot.land(ok, if ok { self.held } else { [0; 2] });
            }
            self.slot.ask(fence);
        }
        if !t.present {
            self.s.off();
            self.rays_moved();
            if joined {
                self.slot.off(E);
                self.slot.off(L);
                self.truth = [false; 2];
                if self.slot.copies_pending() && self.slot.copy(false, [false; 2]) {
                    self.held = self.kept;
                }
            }
            return Ok(*self.s.last());
        }
        if !t.same || t.jitter != self.jitter {
            self.rays_moved();
        }
        self.jitter = t.jitter;
        let read = self.s.read;
        let jitter = self.s.jitter[0];
        let corner = self.s.corner;
        let owed = t.owed.unwrap_or(self.slot.owed);
        let r = Reads {
            same: t.same,
            jitter: [t.jitter, 0],
            gated: t.gated,
            shadows: t.shadows,
            light_made: t.made,
            content_same: t.content_same,
            world_columns: t.world_columns,
            corner: [t.corner, 0],
            receivers_same: t.receivers_same,
            forgets: t.forgets,
            owed,
        };
        let content = self.s.content;
        let p = self.s.plan(r);
        self.forgot |= p.casts() && t.forgets;
        // A view, world or surface moved, or rays the device no longer holds: the eye casts
        // from roll 0, the grid laid anew, its holes walked afresh.
        if !(t.same && read) && (p.kind != FrameKind::cast || !p.lays || p.roll != Some(0)) {
            return Err("a change cast other than from roll 0");
        }
        // The jitter moved alone: the pixels are cast again, no parked ray resumed.
        if t.same && read && t.jitter != jitter && (!p.casts() || p.resumes) {
            return Err("the jitter moved and the pixels not cast afresh");
        }
        if p.casts() && p.resumes && !(t.same && read) {
            return Err("a cast resumed across a change");
        }
        // Owed work, where its observer may run, runs this frame.
        if owed[E] && !p.eye_runs() {
            return Err("the eye owed and not run");
        }
        if owed[L] && t.shadows && t.made && p.light.is_none() {
            return Err("the light owed and not cast");
        }
        // The light is due exactly when what it reads says so.
        let held = t.content_same && content;
        let due = t.shadows
            && t.made
            && (!held
                || [t.corner, 0] != corner
                || !t.receivers_same
                || t.world_columns
                || owed[L]
                || p.light
                    .is_some_and(|l| l.moved & 1 << LightMoved::footprint as u32 != 0));
        if due != p.light.is_some() {
            return Err("the light due otherwise than its reads say");
        }
        if let Some(l) = p.light {
            if l.moved & 1 << LightMoved::footprint as u32 != 0 && !(t.same && read) {
                return Err("a footprint owed where the eye moved");
            }
            if l.carries && (!held || l.slid || t.world_columns || self.forgot) {
                return Err("parked light rays carried across a change");
            }
        }
        if p.light.is_some() {
            self.forgot = false;
        }
        if self.s.gated() && t.whole {
            self.s.whole();
        }
        let stood = !p.casts() && p.light.is_none() && !p.queue;
        let ran = [p.eye_runs(), p.light.is_some()];
        let mut owe = [0u32; 2];
        if !stood && !t.dropped {
            // The eye's walks: every pixel a cast takes is walked; a cast that resumes goes on
            // from its parked ray, any other sets it to none; the queue leaves it parked or
            // ends it. The queue alone walks the list the last run filled.
            let mut listed = [false; REGIONS];
            if p.casts() {
                let taken: [bool; REGIONS] =
                    core::array::from_fn(|k| k == CLAIMED || !self.s.gated());
                for k in 0..REGIONS {
                    if !taken[k] {
                        continue;
                    }
                    if p.resumes && self.parked[k] == Park::Gone {
                        return Err("a parked eye ray resumed across a change");
                    }
                    self.parked[k] = if t.parks { Park::Now } else { Park::No };
                    listed[k] = t.parks;
                }
            } else if p.queue {
                let (Some(from), Some(f)) = (p.from, self.filled) else {
                    return Err("the eye's queue alone with no list filled");
                };
                if from != ray::FROM_OWED + f {
                    return Err("the eye's queue alone took a list its last run did not fill");
                }
                let names = self.lists[f as usize];
                for ((parked, listed), named) in self.parked.iter_mut().zip(&mut listed).zip(names)
                {
                    if named {
                        if *parked != Park::Now {
                            return Err("a parked eye ray resumed across a change");
                        }
                        *parked = if t.parks { Park::Now } else { Park::No };
                        *listed = t.parks;
                    }
                }
            }
            if ran[E] {
                let f = self.s.owed_list();
                self.lists[f as usize] = listed;
                self.filled = Some(f);
                owe[E] = u32::from(listed.contains(&true));
            }
            owe[L] = u32::from(t.light_owed);
            for o in 0..2 {
                if ran[o] {
                    self.kept[o] = owe[o];
                }
            }
        }
        if joined {
            if stood {
                if self.slot.copies_pending() && self.slot.copy(false, [false; 2]) {
                    self.held = self.kept;
                }
            } else if self.slot.copy(false, ran) {
                self.held = self.kept;
            }
            for o in 0..2 {
                if ran[o] {
                    self.truth[o] = owe[o] != 0 || (t.dropped && !stood);
                }
            }
        }
        if t.dropped && !stood {
            self.s.dropped();
            if joined {
                self.slot.dropped();
            }
        }
        Ok(p)
    }

    /// The light's cast let go (cast.rs `light_made`).
    fn light_off(&mut self) {
        self.s.light_off();
        self.slot.off(L);
        self.truth[L] = false;
    }

    /// A wake is coming for owed work: a frame asked for, or a mapping whose fence wakes the turn.
    fn waking(&self) -> bool {
        self.slot.wants_frame(self.s.on()) || self.slot.wakes(true)
    }

    /// Where an observer that may run owes work on the device, a wake is coming.
    fn holds(&self) -> bool {
        let on = self.s.on();
        !(0..2).any(|o| on[o] && self.truth[o]) || self.waking()
    }

    /// The world as the exploration keys it: the readback's generations by whether the copy
    /// out holds each observer's last (`Readback::current`), which alone its steps read, and
    /// the roll's casts between its first two and its last two as one, which step alike.
    fn key(&self) -> World {
        let mut w = *self;
        for o in 0..2 {
            w.slot.carried[o] =
                w.slot.carried[o].map(|c| if c == w.slot.cast[o] { 0 } else { u32::MAX });
            w.slot.cast[o] = 0;
        }
        if (2..beam::ROLLS - 1).contains(&w.s.rolls) {
            w.s.rolls = 2;
        }
        w
    }
}

/// What a frame reads that its plan reads, an alphabet's each (`Alphabet`), from `w`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Alphabet {
    /// The eye's reads whole, the light's on, off or not made and its content moved or not,
    /// the observers' owed work the readback's, the walks parking or not: the eye, its parked
    /// rays and the readback joined.
    Joined,
    /// Every read whole, the eye's and the light's, the owed work any: the light's decisions,
    /// no readback stepped.
    Light,
}

/// Every turn a frame may take from `w` in `alphabet`: each read either way, but those the
/// step reads only where it reads them (the landing where a mapping is out, the eye's and the
/// light's parking where they run, the drop where something is submitted, drawn whole where
/// gated).
fn turns(w: &World, alphabet: Alphabet) -> Vec<Turn> {
    let joined = alphabet == Alphabet::Joined;
    let landings: &[u8] = if joined && w.slot.mapped {
        &[0, 1, 2]
    } else {
        &[0]
    };
    let none = Turn {
        landed: 0,
        present: false,
        same: false,
        jitter: w.jitter,
        gated: false,
        shadows: false,
        made: false,
        content_same: true,
        world_columns: false,
        corner: w.s.corner[0],
        receivers_same: true,
        forgets: false,
        owed: None,
        parks: false,
        light_owed: false,
        whole: false,
        dropped: false,
    };
    let mut reads = Vec::new();
    let bits = if joined { 6 } else { 10 };
    for b in 0..1u32 << bits {
        let bit = |k: u32| b >> k & 1 != 0;
        let t = Turn {
            present: true,
            same: bit(0),
            jitter: u32::from(bit(1)),
            gated: bit(2),
            shadows: bit(3),
            made: bit(4),
            content_same: bit(5),
            ..none
        };
        if joined {
            reads.push(t);
            continue;
        }
        for owed in [[false, false], [true, false], [false, true], [true, true]] {
            reads.push(Turn {
                world_columns: bit(6),
                corner: i32::from(bit(7)),
                receivers_same: bit(8),
                forgets: bit(9),
                owed: Some(owed),
                ..t
            });
        }
    }
    let mut all = Vec::new();
    for &landed in landings {
        all.push(Turn {
            landed,
            owed: (!joined).then_some([false; 2]),
            ..none
        });
        for &t in &reads {
            let t = Turn { landed, ..t };
            // What the plan makes of it decides which of the rest the step reads.
            let mut probe = *w;
            let Ok(p) = probe.turn(t) else {
                all.push(t);
                continue;
            };
            let stood = !p.casts() && p.light.is_none() && !p.queue;
            for parks in [false, true] {
                if parks && !(joined && p.eye_runs()) {
                    continue;
                }
                for light_owed in [false, true] {
                    if light_owed && !(joined && p.light.is_some()) {
                        continue;
                    }
                    for whole in [false, true] {
                        if whole && !probe.s.gated() {
                            continue;
                        }
                        for dropped in [false, true] {
                            if dropped && stood {
                                continue;
                            }
                            all.push(Turn {
                                parks,
                                light_owed,
                                whole,
                                dropped,
                                ..t
                            });
                        }
                    }
                }
            }
        }
    }
    all
}

/// Every world reached from the first frame's by turns of `alphabet` and the light's cast let
/// go between any two, explored whole: the step from each world reached is taken once, and
/// the roll's middle casts once for all of them (`World::key`), from the roll's first middle
/// cast and its last. Each world reached holds the wake invariant; each step its own.
fn reached(alphabet: Alphabet) -> Result<Vec<World>, String> {
    let start = World::new();
    let mut seen = FnvHashSet::from_iter([start.key()]);
    let mut all = vec![start];
    let mut k = 0;
    while k < all.len() {
        let w = all[k];
        k += 1;
        let mut from = vec![w];
        if w.s.rolls == 2 {
            let mut last = w;
            last.s.rolls = beam::ROLLS - 2;
            from.push(last);
        }
        for w in from {
            let mut next = Vec::new();
            for t in turns(&w, alphabet) {
                let mut a = w;
                if let Err(e) = a.turn(t) {
                    return Err(format!("{e}: from {w:?} by {t:?}"));
                }
                next.push(a);
            }
            let mut a = w;
            a.light_off();
            next.push(a);
            for a in next {
                if !a.holds() {
                    return Err(format!("owed and no wake: {a:?}"));
                }
                if seen.insert(a.key()) {
                    all.push(a.key());
                }
            }
        }
    }
    Ok(all)
}

/// The worlds each alphabet reaches, explored once for every test that reads them.
fn worlds(alphabet: Alphabet) -> &'static [World] {
    type Reached = std::sync::OnceLock<Result<Vec<World>, String>>;
    static JOINED: Reached = Reached::new();
    static LIGHT: Reached = Reached::new();
    let once = if alphabet == Alphabet::Joined {
        &JOINED
    } else {
        &LIGHT
    };
    match once.get_or_init(|| reached(alphabet)) {
        Ok(all) => all,
        Err(e) => panic!("{e}"),
    }
}

/// Every state the eye, its parked rays and the readback reach together holds: owed work has
/// a wake; a change casts from roll 0; parked rays are dropped on a change and never resumed
/// across one; the queue alone takes the list its last run filled; owed work runs.
#[test]
fn the_eye_and_its_owed_work_hold_in_every_state() {
    let all = worlds(Alphabet::Joined);
    std::println!("{} worlds", all.len());
}

/// Every state the light's decisions reach holds: it is due exactly when its reads say so,
/// its parked rays carried only within its epoch, under the same box, no changed column and
/// no forgetting between; the world's receivers added only within the epoch (`Schedule::plan`'s
/// `ensure`).
#[test]
fn the_light_holds_in_every_state() {
    let all = worlds(Alphabet::Light);
    std::println!("{} worlds", all.len());
}

/// The turns a standing scene takes to rest from any world reached: the rolled lattice's
/// `beam::ROLLS` casts, the first of them where the rays hold nothing the frame reads; after
/// the last cast, which draws the frame whole and starts the jitter's sequence again
/// (gpu/resolve.rs), the sequence's `JITTERS - 1` frames, each casting the pixels at its place;
/// and the last run's owed work read back: its copy waiting a turn where a mapping was out,
/// copied alone by the turn after (`Readback::copies_pending`), mapped by the next and landing
/// by the one after that, each mapping landing by the turn after it is asked.
const REST: usize = beam::ROLLS as usize + (JITTERS as usize - 1) + 3;

/// A scene that stands from `start`: the view, the world, the light and the settings stand
/// (`shadows`), the jitter moving only while the resolve's sequence asks, every mapping landing
/// the turn after it is asked, no walk leaving work parked, and no frame dropped; where
/// `animating`, what the scene animates alone (an idle actor) steps every turn, its tiles
/// marked and the frame drawn through the gate, the jitter held by it (gpu/resolve.rs `asks`).
/// The turns it takes until nothing asks a frame and no wake is coming; then, the animation
/// stepping on, a turn casts no pixel, runs no queue, casts no light and owes nothing.
fn rests(start: World, shadows: bool, animating: bool) -> Result<usize, String> {
    let mut w = start;
    // The jitter's frames left of its sequence, the most a start may hold.
    let mut gather = JITTERS - 1;
    let corner = w.s.corner[0];
    let standing = |w: &World, jitter: u32| Turn {
        landed: u8::from(w.slot.mapped),
        present: true,
        same: true,
        jitter,
        gated: animating,
        shadows,
        made: shadows,
        content_same: true,
        world_columns: false,
        corner,
        receivers_same: true,
        forgets: false,
        owed: None,
        parks: false,
        light_owed: false,
        whole: !animating,
        dropped: false,
    };
    for n in 0..=REST {
        if !w.s.rolling() && gather == 0 && !w.waking() {
            for _ in 0..JITTERS {
                let p = w.turn(standing(&w, w.jitter)).map_err(String::from)?;
                if p.casts() || p.light.is_some() || p.queue || w.s.rolling() || w.waking() {
                    return Err(format!(
                        "asks a frame after it rested (animating {animating}): {w:?}"
                    ));
                }
            }
            return Ok(n);
        }
        let jitter = if gather > 0 { 1 - w.jitter } else { w.jitter };
        gather = gather.saturating_sub(1);
        let p = w.turn(standing(&w, jitter)).map_err(String::from)?;
        if p.kind == FrameKind::cast {
            gather = JITTERS - 1;
        }
    }
    Err(format!("no rest within {REST} turns from {start:?}: {w:?}"))
}

#[test]
fn a_standing_scene_rests_within_its_bound() {
    let mut most = 0;
    for &w in worlds(Alphabet::Joined)
        .iter()
        .chain(worlds(Alphabet::Light))
    {
        for (shadows, animating) in [(false, false), (true, false), (false, true), (true, true)] {
            most = most.max(rests(w, shadows, animating).unwrap_or_else(|e| panic!("{e}")));
        }
    }
    assert!(
        most <= REST,
        "{most} turns to rest past the bound of {REST}"
    );
}
