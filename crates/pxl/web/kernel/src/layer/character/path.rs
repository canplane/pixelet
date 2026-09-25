// pxl-kernel/layer/character/path.rs

//! Pure companion-movement math — no renderer of any kind. Positioning/timing
//! logic is kept independent of the renderer on purpose, which is how it
//! survived the move from three to the kernel's actors
//! (crates/pxl/web/kernel/src/layer/model/store.rs) unchanged.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

/// Bearing from `from` to `to`, in radians (0 = +x).
pub fn heading_to(from: Vec2, to: Vec2) -> f64 {
    libm::atan2(to.y - from.y, to.x - from.x)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkPlan {
    pub start: Vec2,
    pub target: Vec2,
    /// The clock's elapsed time at the moment this plan was created.
    pub start_time: f64,
    /// Seconds the whole start->target traversal takes, regardless of
    /// distance — see sample_walk_plan's own doc comment for why.
    pub duration: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkSample {
    pub x: f64,
    pub y: f64,
    /// Direction of travel, or None once arrived — callers should fall back to
    /// some other default facing (e.g. facing the camera) in that case.
    pub heading: Option<f64>,
    pub arrived: bool,
}

/// Samples a straight-line move from `plan.start` to `plan.target` that always
/// takes exactly `plan.duration` seconds — distance-independent by design. A
/// companion catching up after a pan should reappear at your side on roughly
/// the same beat whether you panned 1 unit or 1000; a fixed real-world walking
/// *speed* would instead make a big pan take proportionally, sometimes
/// absurdly, longer.
pub fn sample_walk_plan(plan: &WalkPlan, now: f64) -> WalkSample {
    let elapsed = now - plan.start_time;
    let frac = if plan.duration > 0.0 { (elapsed / plan.duration).clamp(0.0, 1.0) } else { 1.0 };
    let arrived = frac >= 1.0;
    WalkSample {
        x: plan.start.x + (plan.target.x - plan.start.x) * frac,
        y: plan.start.y + (plan.target.y - plan.start.y) * frac,
        heading: if arrived { None } else { Some(heading_to(plan.start, plan.target)) },
        arrived,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleTracker {
    /// Most recent raw target seen, whether or not it's stopped moving yet.
    pub last_seen: Vec2,
    /// When `last_seen` last actually changed.
    pub last_change_time: f64,
    /// The last target this tracker has reported as settled.
    pub settled: Vec2,
}

pub fn init_settle_tracker(initial: Vec2, now: f64) -> SettleTracker {
    SettleTracker {
        last_seen: initial,
        last_change_time: now,
        settled: initial,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleUpdate {
    pub tracker: SettleTracker,
    pub just_settled: bool,
    pub moved: bool,
}

/// Tracks a moving target (e.g. map_center) and reports the instant it stops
/// changing, instead of reacting to any one specific gesture. A drag, a
/// click-to-navigate tween, a GPS recenter, a keyboard nudge — anything that
/// moves the target eventually stops changing it, and that's the one moment
/// this fires `just_settled`, regardless of which of those caused it.
/// (camera_status: DRAG/IDLE was tried first — it only fires for drag/
/// rotate/pinch, so a click-driven camera animation that never touches
/// pointer-drag state never triggered a catch-up at all.)
pub fn update_settle_tracker(tracker: &SettleTracker, target: Vec2, now: f64, settle_delay: f64) -> SettleUpdate {
    let moved = target.x != tracker.last_seen.x || target.y != tracker.last_seen.y;
    let last_seen = if moved { target } else { tracker.last_seen };
    let last_change_time = if moved { now } else { tracker.last_change_time };

    let stable_for = now - last_change_time;
    let differs_from_settled = last_seen.x != tracker.settled.x || last_seen.y != tracker.settled.y;

    if stable_for >= settle_delay && differs_from_settled {
        return SettleUpdate {
            tracker: SettleTracker {
                last_seen,
                last_change_time,
                settled: last_seen,
            },
            just_settled: true,
            moved,
        };
    }
    SettleUpdate {
        tracker: SettleTracker {
            last_seen,
            last_change_time,
            settled: tracker.settled,
        },
        just_settled: false,
        moved,
    }
}
