// pxl-geo-bake/terrain/pipeline/overture/between.rs

//! Overture's linear referencing: a rule holds over a `between` range of a
//! segment, or the whole of it.

use crate::terrain::CELL_U;
use pxl_spec::space::coord::adapter::epsg3857::{mer_from_u, G3857};
use serde_json::Value;

pub type Range = [f64; 2];

fn range_of(rule: &Value) -> Option<Range> {
    let between = rule["between"].as_array().map(|b| {
        (
            b.first().and_then(Value::as_f64),
            b.get(1).and_then(Value::as_f64),
        )
    });
    let [a, b] = match between {
        Some((Some(a), Some(b))) => [a.clamp(0.0, 1.0), b.clamp(0.0, 1.0)],
        _ => [0.0, 1.0],
    };
    (a < b).then_some([a, b])
}

/// What a ground cell spans of a way `length` long in DB coordinates: a gap
/// in a flag shorter than that shows on no cell, so the ranges either side are one.
pub fn cell_of(length: f64) -> f64 {
    mer_from_u(CELL_U) / length.max(f64::EPSILON)
}

/// A column's rules, parsed once: each the range it holds over, and itself.
pub struct Rules(Vec<(Range, Value)>);

impl Rules {
    /// An unparseable column says nothing.
    pub fn of(column: Option<&str>) -> Rules {
        let all = match column.map(serde_json::from_str::<Value>) {
            Some(Ok(Value::Array(all))) => all,
            _ => Vec::new(),
        };
        Rules(
            all.into_iter()
                .filter_map(|r| Some((range_of(&r)?, r)))
                .collect(),
        )
    }

    /// Where a flag among `wanted` holds, ranges `cell` apart joined (`cell_of`).
    pub fn flagged(&self, wanted: &[&str], cell: f64) -> Vec<Range> {
        let has = |r: &Value| {
            r["values"].as_array().is_some_and(|v| {
                v.iter()
                    .any(|v| v.as_str().is_some_and(|v| wanted.contains(&v)))
            })
        };
        merged(
            self.0
                .iter()
                .filter(|(_, r)| has(r))
                .map(|(range, _)| *range)
                .collect(),
            cell,
        )
    }

    /// Where every rule begins and ends.
    pub fn bounds(&self) -> impl Iterator<Item = f64> + '_ {
        self.0.iter().flat_map(|(range, _)| *range)
    }

    /// The value of the first rule that holds at `at`.
    pub fn at(&self, at: f64) -> Option<&Value> {
        self.0
            .iter()
            .find(|([a, b], _)| (*a..=*b).contains(&at))
            .map(|(_, r)| &r["value"])
    }
}

pub fn merged(mut ranges: Vec<Range>, gap: f64) -> Vec<Range> {
    ranges.sort_by(|x, y| x[0].total_cmp(&y[0]));
    let mut merged: Vec<Range> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r[0] <= last[1] || r[0] - last[1] < gap => last[1] = last[1].max(r[1]),
            _ => merged.push(r),
        }
    }
    merged
}

/// What of `[0, 1]` no range in `less` covers.
pub fn uncovered(less: &[Range]) -> Vec<Range> {
    let mut out = Vec::new();
    let mut from = 0.0;
    for &[a, b] in less {
        if a > from {
            out.push([from, a]);
        }
        from = f64::max(from, b);
    }
    if from < 1.0 {
        out.push([from, 1.0]);
    }
    out
}

/// A line by its length so far at each point. Fractions are of the planar
/// 3857 length where Overture's are geodesic; over one segment the
/// difference is far under a cell.
pub struct Line<'a> {
    points: &'a [G3857],
    cum: Vec<f64>,
}

impl<'a> Line<'a> {
    pub fn of(points: &'a [G3857]) -> Option<Line<'a>> {
        let mut cum = Vec::with_capacity(points.len());
        let mut len = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                let q = points[i - 1];
                len += libm::hypot(p[0] - q[0], p[1] - q[1]);
            }
            cum.push(len);
        }
        (points.len() >= 2 && len > 0.0).then_some(Line { points, cum })
    }

    pub fn length(&self) -> f64 {
        *self.cum.last().unwrap()
    }

    pub fn at(&self, d: f64) -> G3857 {
        // its own ends bit for bit, as the segments meeting there carry them
        if d <= 0.0 {
            return self.points[0];
        }
        if d >= self.length() {
            return self.points[self.points.len() - 1];
        }
        let i = self
            .cum
            .partition_point(|&c| c < d)
            .clamp(1, self.points.len() - 1);
        let (p, q) = (self.points[i - 1], self.points[i]);
        let span = self.cum[i] - self.cum[i - 1];
        let t = if span > 0.0 {
            (d - self.cum[i - 1]) / span
        } else {
            0.0
        };
        [
            p[0] + (q[0] - p[0]) * t,
            p[1] + (q[1] - p[1]) * t,
            p[2] + (q[2] - p[2]) * t,
        ]
    }

    pub fn between(&self, from: f64, to: f64) -> Vec<G3857> {
        let (from, to) = (from * self.length(), to * self.length());
        let mut out = vec![self.at(from)];
        out.extend(
            self.points
                .iter()
                .zip(&self.cum)
                .filter(|(_, &c)| c > from && c < to)
                .map(|(p, _)| *p),
        );
        out.push(self.at(to));
        out
    }
}
