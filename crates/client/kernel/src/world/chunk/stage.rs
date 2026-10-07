// pxl-kernel/world/chunk/stage.rs

//! The world's part of the tick: in the visible phase it composes the frame
//! and runs the table's pass; in the report phase it hands the frame's
//! numbers to the profiler.

use pxl_spec::space::coord::v3::sub3;
use xpute_guest::sched::edge::{notified, yield_if_spent};

use crate::sched::edge::{request_world_tick, world_edge};
use crate::sched::tick::{TickContext, HARD_SLACK_MS};
use crate::world::chunk::decor::build::DecorHide;
use crate::world::chunk::table::{builds, Frame};
use crate::world::view::frame::pose::globe;
use crate::world::view::frame::pose::{sea_under_eye, view, view_resize};
use crate::{kernel, with_world};
use xpute_guest::clock::now;
use xpute_guest::global::Global;

struct CoreTick {
    visible_pass: Option<f64>,
    draw: bool,
}

static CORE_TICK: Global<CoreTick> = Global::new();

fn core_tick() -> &'static mut CoreTick {
    CORE_TICK.get(|| CoreTick { visible_pass: None, draw: false })
}

pub fn take_draw() -> bool {
    core::mem::take(&mut core_tick().draw)
}

pub fn core_tick_visible_step(ctx: &mut TickContext) {
    let c = core_tick();
    c.visible_pass = None;
    view_resize(ctx.state.width, ctx.state.height);
    let v = view();

    let anchor = sea_under_eye();
    let e = v.pose.eye;
    let frame = Frame {
        anchor,
        eye: e,
        frustum: v.frustum.about(anchor),
        camera: sub3(&e, &anchor),
        delta_s: ctx.delta,
        remaining_ms: ctx.budget.remaining(),
        hard_slack_ms: HARD_SLACK_MS,
        globe: Some(globe()),
    };

    let t0 = now();
    let draw = with_world(|_, sched, scene| sched.tick(frame, &now, scene)).unwrap_or(false);
    // Decor lists after the chunks have listed theirs.
    {
        let k = kernel();
        let edits = crate::control::interaction::edit::render_edits();
        let hide = edits.hide.filter(|_| edits.hide_in.1).map(DecorHide::Placed).or(edits.scattered.map(DecorHide::Scattered));
        crate::world::chunk::decor::cell::step(&crate::world::chunk::decor::kind::KINDS, &k.objects, &k.sched, &mut k.scene.draw, hide, frame.eye, &ctx.budget);
    }
    crate::world::chunk::place::step(ctx);
    let scene = &mut kernel().scene;
    scene.billboards.draw(&mut scene.draw, &scene.models, frame.eye);
    if draw {
        c.draw = true;
        ctx.invalidate();
    }
    c.visible_pass = Some(now() - t0);
}

/// Advances the builds on what the quota leaves after the tick, at least one
/// step a turn, and waits for the next tick once a step does nothing.
pub async fn build_task() {
    loop {
        notified(builds).await;
        loop {
            yield_if_spent(world_edge).await;
            let budget = world_edge().remaining();
            let worked = with_world(|world, sched, _| sched.build_step(world, budget)).unwrap_or(false);
            if !worked {
                break;
            }
            request_world_tick();
        }
    }
}

/// How long this turn's chunks took; none where the stage was empty.
pub fn visible_pass() -> Option<f64> {
    core_tick().visible_pass
}
