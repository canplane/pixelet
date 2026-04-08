// @pxl/spec/lib.rs

pub mod space;

#[cfg(test)]
mod tests {
    use crate::space::pvf3::{ivec, fvec, projection};

    #[test]
    fn ivec_round_trip() {
        // gather/scatter is lossless within lane bounds
        let v = ivec::gather(3, -2, 1);
        assert_eq!(ivec::x(v), 3);
        assert_eq!(ivec::y(v), -2);
        assert_eq!(ivec::z(v), 1);
    }

    #[test]
    fn ivec_sign_extension() {
        // max positive per lane
        let v = ivec::gather((1 << 23) - 1, (1 << 23) - 1, (1 << 15) - 1);
        assert_eq!(ivec::x(v), (1 << 23) - 1);
        assert_eq!(ivec::z(v), (1 << 15) - 1);
        // min negative per lane (-1 stored in 24 bits = all bits set)
        let w = ivec::gather(-1, -1, -1);
        assert_eq!(ivec::x(w), -1);
        assert_eq!(ivec::y(w), -1);
        assert_eq!(ivec::z(w), -1);
    }

    #[test]
    fn fvec_round_trip() {
        let v = fvec::gather(512, 256, 1);
        assert_eq!(fvec::x(v), 512);
        assert_eq!(fvec::y(v), 256);
        assert_eq!(fvec::z(v), 1);
    }

    #[test]
    fn project_unproject_identity() {
        let p = projection::project(1.5, -0.25, 3.0);
        let [rx, ry, rz] = projection::unproject(p.i, p.f);
        assert!((rx - 1.5).abs() < 1e-6, "x: {rx}");
        assert!((ry - -0.25).abs() < 1e-6, "y: {ry}");
        assert!((rz - 3.0).abs() < 1e-6, "z: {rz}");
    }

    #[test]
    fn epsg3857_round_trip() {
        use crate::space::adapter::epsg3857;
        // Project Tokyo, then unproject back
        let [x, y, z] = [15_553_000.0f64, 4_247_000.0, 0.0]; // approx meters
        let p = epsg3857::project(x, y, z);
        let [rx, ry, rz] = epsg3857::unproject(p.i, p.f);
        assert!((rx - x).abs() < 0.01, "x: {rx} vs {x}");
        assert!((ry - y).abs() < 0.01, "y: {ry} vs {y}");
        let _ = rz;
    }

    #[test]
    fn epsg4326_round_trip() {
        use crate::space::adapter::epsg4326;
        let src = [139.6917f64, 35.6895, 0.0]; // Tokyo
        let p = epsg4326::project(src);
        let out = epsg4326::unproject(p.i, p.f);
        assert!((out[0] - src[0]).abs() < 1e-4, "lng: {} vs {}", out[0], src[0]);
        assert!((out[1] - src[1]).abs() < 1e-4, "lat: {} vs {}", out[1], src[1]);
    }

    #[test]
    fn haversine_dist() {
        use crate::space::adapter::epsg4326;
        // London -> Paris ≈ 340 km
        let london = [-0.1276f64, 51.5074, 0.0];
        let paris = [2.3522f64, 48.8566, 0.0];
        let d = epsg4326::dist(london, paris);
        assert!((d - 340_000.0).abs() < 5_000.0, "dist: {d}");
    }

    #[test]
    fn yup_zup_inverse() {
        use crate::space::adapter::yup;
        let v = [1.0f64, 2.0, 3.0];
        let y = yup::yup(v);
        let back = yup::zup(y);
        assert!((back[0] - v[0]).abs() < 1e-12);
        assert!((back[1] - v[1]).abs() < 1e-12);
        assert!((back[2] - v[2]).abs() < 1e-12);
    }

    #[test]
    fn project_negative_continuity() {
        // -0.2 => i=-1, f=0.8 (floor continuity across negative domain)
        let p = projection::project(-0.2, 0.0, 0.0);
        assert_eq!(ivec::x(p.i), -1);
        let fx = fvec::to_real(fvec::x(p.f));
        assert!((fx - 0.8).abs() < 1e-3, "f.x: {fx}");
    }
}
