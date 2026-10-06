// pxl-geo-bake/deck/way.rs

//! A way as read, and the line it runs along.

use crate::segment::Segment;

type P = [f64; 2];

/// A polyline with its length so far at each point.
pub struct Polyline {
    pub points: Vec<P>,
    along: Vec<f64>,
}

impl Polyline {
    pub fn of(points: impl IntoIterator<Item = P>) -> Polyline {
        let points: Vec<P> = points.into_iter().collect();
        let mut along = vec![0.0; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + libm::hypot(points[i][0] - points[i - 1][0], points[i][1] - points[i - 1][1]);
        }
        Polyline { points, along }
    }

    pub fn length(&self) -> f64 {
        self.along.last().copied().unwrap_or(0.0)
    }

    /// The point `d` along it, in DB coordinates.
    pub fn at(&self, d: f64) -> P {
        let i = self.along.partition_point(|&x| x < d).clamp(1, self.points.len() - 1);
        let (a, b) = (self.points[i - 1], self.points[i]);
        let seg = self.along[i] - self.along[i - 1];
        let t = if seg > 0.0 { (d - self.along[i - 1]) / seg } else { 0.0 };
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }
}

/// A way as read: its row and its line.
pub struct Way {
    pub row: Segment,
    pub line: Polyline,
}

impl Way {
    pub fn of(row: Segment) -> Option<Way> {
        (row.points.len() >= 2).then(|| Way {
            line: Polyline::of(row.points.iter().map(|p| [p[0], p[1]])),
            row,
        })
    }
}
