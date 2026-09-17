// pxl_core/sched/priority.test.rs

use super::*;
use xpute_core::kit::mat4::{mat4, mat4_mul, mat4_rotation_z, mat4_translation, Vec3};

const CAMERA: Vec3 = Vec3 { x: 0.0, y: -300.0, z: 200.0 };

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} is not {b}");
}

#[test]
fn lead_point_at_rest_the_target_sits_at_the_world_origin_so_the_lead_is_the_camera() {
    // The frame has caught up: it is translated so the target is at the origin.
    let mut frame = mat4();
    mat4_translation(&mut frame, -1000.0, -2000.0, -30.0);
    let lead = lead_point(&CAMERA, 1000.0, 2000.0, 30.0, &frame);
    close((lead.x - CAMERA.x).hypot(lead.y - CAMERA.y).hypot(lead.z - CAMERA.z), 0.0);
}

#[test]
fn lead_point_mid_pan_the_lead_is_ahead_of_the_camera_by_the_travel_still_to_come() {
    // Frame still showing (1000, 2000) while the target moved on to (1100, 2000).
    let mut frame = mat4();
    mat4_translation(&mut frame, -1000.0, -2000.0, -30.0);
    let lead = lead_point(&CAMERA, 1100.0, 2000.0, 30.0, &frame);
    close(lead.x - CAMERA.x, 100.0);
    close(lead.y - CAMERA.y, 0.0);
}

#[test]
fn lead_point_the_frame_s_rotation_carries_through_so_ahead_matches_the_screen() {
    // A quarter turn of azimuth: local +X is world +Y.
    let (mut r, mut t, mut frame) = (mat4(), mat4(), mat4());
    mat4_rotation_z(&mut r, core::f64::consts::PI / 2.0);
    mat4_translation(&mut t, -1000.0, -2000.0, -30.0);
    mat4_mul(&mut frame, &r, &t);
    let lead = lead_point(&CAMERA, 1100.0, 2000.0, 30.0, &frame);
    close(lead.x - CAMERA.x, 0.0);
    close(lead.y - CAMERA.y, 100.0);
}
