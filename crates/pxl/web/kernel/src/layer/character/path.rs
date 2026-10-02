// pxl-kernel/layer/character/path.rs

//! Companion movement math, independent of any renderer.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

/// Radians, 0 = +x.
pub fn heading_to(from: Vec2, to: Vec2) -> f64 {
    libm::atan2(to.y - from.y, to.x - from.x)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkPlan {
    pub start: Vec2,
    pub target: Vec2,
    pub start_time: f64,
    /// Fixed whatever the distance (see sample_walk_plan).
    pub duration: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkSample {
    pub x: f64,
    pub y: f64,
    /// None once arrived.
    pub heading: Option<f64>,
    pub arrived: bool,
}

/// Takes `plan.duration` whatever the distance, so the companion catches up
/// on the same beat after a pan of 1 unit or 1000; a fixed speed made big
/// pans absurdly long.
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
    pub last_seen: Vec2,
    pub last_change_time: f64,
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

/// Fires `just_settled` when the target stops changing, whatever moved it.
/// Keying on drag state missed click-driven camera animations entirely.
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
