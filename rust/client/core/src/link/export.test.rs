// pxl-core/link/export.test.rs

use super::*;
use crate::domain::control::camera::state::{camera_state, Intent};
use crate::domain::control::navigation::destination;
use crate::domain::control::param;
use crate::perf::spec::RING_NAMES;
use xpute_gpu_guest::abi::GpuSurfaceClose;
use xpute_kit::wire::xtp::PacketWriter;

fn surface(n: f64) -> Result<u32, Errno> {
    let mut buf = [0u8; 64];
    let len = {
        let mut w = PacketWriter::new(&mut buf, 1);
        w.f64(Some(n));
        w.finish().expect("the buffer holds one number")
    };
    GpuSurfaceClose::read(&buf[..len as usize])
        .map(|p| p.surface)
        .map_err(|e| e.errnum)
}

#[test]
fn number_checked() {
    crate::on_core(|| {
        assert_eq!(surface(13.0), Ok(13));
        for n in [f64::NAN, f64::INFINITY, -1.0, 2.5, 1e20] {
            assert_eq!(surface(n), Err(Errno::EINVAL), "{n} is no u32");
        }
    });
}

fn go_to(from: f64, dist: Option<f64>, azimuth: Option<f64>, polar: Option<f64>) -> Intent {
    use crate::domain::control::camera::controller::tracked_end;
    use crate::domain::control::camera::fixture::place;
    tracked_end();
    place([0.0, 0.0, 0.0], from, 1.0, 0.5);
    let mut buf = [0u8; 128];
    let len = {
        let mut w = PacketWriter::new(&mut buf, 5);
        w.f64(Some(127.0))
            .f64(Some(37.5))
            .f64(dist)
            .f64(azimuth)
            .f64(polar);
        w.finish().expect("the buffer holds five numbers")
    };
    let mut s = State {
        last_snapshot: Default::default(),
        snapshot_due: false,
    };
    assert_eq!(
        ui::apply(
            &mut s,
            abi::Command::CAMERA_GOTO as u32,
            &Some(&buf[..len as usize])
        )
        .map(|r| r.map(|r| r.val))
        .map_err(|e| e.errnum),
        Ok(Some(0))
    );
    destination::cancel();
    camera_state().intent
}

/// A view field the command asks for is applied as the controls would take
/// it, after the camera has come in. A field left out keeps its value, but a
/// view far out still comes in, and a view already nearer stays where it is.
#[test]
fn go_to_asks_after() {
    crate::on_core(|| {
        use crate::domain::control::param::DEFAULT_DIST;
        let pi = core::f64::consts::PI;

        let unasked = go_to(100.0, None, None, None);
        assert_eq!(
            (unasked.dist, unasked.azimuth, unasked.polar),
            (DEFAULT_DIST, 0.5, 1.0)
        );

        let asked = go_to(100.0, Some(50.0), Some(7.0), Some(0.3));
        assert_eq!((asked.dist, asked.polar), (50.0, 0.3));
        assert!((asked.azimuth - (7.0 - 2.0 * pi)).abs() < 1e-12, "wrapped");

        assert_eq!(
            go_to(100.0, Some(1e9), None, Some(-1.0)).dist,
            param::MAX_DIST
        );
        assert_eq!(go_to(100.0, None, None, Some(-1.0)).polar, 0.0, "clamped");
        assert_eq!(go_to(10.0, None, None, None).dist, 10.0, "nearer stays");
    });
}

/// A ring past RING_NAMES is refused.
#[test]
fn ring_past_the_names_refused() {
    crate::on_core(|| {
        let ring = RING_NAMES.len() as u32;
        assert_eq!(
            perf::ring(abi::PerfRings { ring })
                .map_err(|e| e.errnum)
                .err(),
            Some(Errno::EINVAL)
        );
    });
}
