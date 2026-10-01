// pxl-geo-bake/terrain/pipeline/overture/between.rs

//! Overture's linear referencing: a rule on a segment holds over a `between`
//! range of its length, as fractions of it, or over the whole of it where it
//! has none. A flag (`road_flags`, `rail_flags`) and a level (`level_rules`)
//! are both such rules, so one segment of a subway line can run over a
//! bridge, along the surface and into a tunnel. The bake and the deck solve
//! (deck/) cut a segment the same way, here.

use pxl_spec::space::adapter::epsg3857::G3857;
use serde_json::Value;

use crate::wkb::WkbGeometry;

/// A range of a segment's length, as fractions of it.
pub type Range = [f64; 2];

/// The rules in a JSON column, or none where it does not parse: a column
/// that cannot be read says nothing, which paints the segment as it was
/// painted before any were read.
fn rules(column: Option<&str>) -> Vec<Value> {
    column.and_then(|c| serde_json::from_str::<Value>(c).ok()).and_then(|v| v.as_array().cloned()).unwrap_or_default()
}

/// The range a rule holds over.
fn range_of(rule: &Value) -> Option<Range> {
    let between = rule["between"].as_array().map(|b| (b.first().and_then(Value::as_f64), b.get(1).and_then(Value::as_f64)));
    let [a, b] = match between {
        Some((Some(a), Some(b))) => [a.clamp(0.0, 1.0), b.clamp(0.0, 1.0)],
        _ => [0.0, 1.0],
    };
    (a < b).then_some([a, b])
}

/// The ranges over which any of `wanted` holds among a flags column's rules,
/// sorted and merged.
pub fn flag_ranges(flags: Option<&str>, wanted: &[&str]) -> Vec<Range> {
    let mut ranges: Vec<Range> = rules(flags)
        .iter()
        .filter(|r| r["values"].as_array().is_some_and(|v| v.iter().any(|v| v.as_str().is_some_and(|v| wanted.contains(&v)))))
        .filter_map(range_of)
        .collect();
    ranges.sort_by(|x, y| x[0].total_cmp(&y[0]));
    let mut merged: Vec<Range> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r[0] <= last[1] => last[1] = last[1].max(r[1]),
            _ => merged.push(r),
        }
    }
    merged
}

/// `ranges` with `less` taken out of them.
pub fn without(ranges: &[Range], less: &[Range]) -> Vec<Range> {
    let mut out = Vec::new();
    for &[a, b] in ranges {
        let mut from = a;
        for &[la, lb] in less {
            if lb <= from || la >= b {
                continue;
            }
            if la > from {
                out.push([from, la]);
            }
            from = from.max(lb);
        }
        if from < b {
            out.push([from, b]);
        }
    }
    out
}

/// The value of the rule of a values column (`level_rules`) that holds at
/// fraction `at`, the first that does.
pub fn value_at(column: Option<&str>, at: f64) -> Option<i64> {
    rules(column).iter().find(|r| range_of(r).is_some_and(|[a, b]| (a..=b).contains(&at))).and_then(|r| r["value"].as_i64())
}

/// A line cut wherever a range of any of `sets` starts or ends, each piece
/// marked by which sets it lies inside, with the range of the line it is.
/// A fraction is of the line's planar length in EPSG:3857, where Overture
/// measures its own along the geodesic; over one segment the two differ by
/// the change in Mercator's scale across it, far under a cell.
pub fn split_ranged<const N: usize>(g: WkbGeometry, sets: [&[Range]; N]) -> Vec<([bool; N], Range, WkbGeometry)> {
    let whole = [false; N];
    let line = Line::of(&g.points);
    if sets.iter().all(|s| s.is_empty()) || g.is_polygon_source || line.is_none() {
        return vec![(whole, [0.0, 1.0], g)];
    }
    let line = line.unwrap();
    let mut cuts: Vec<f64> = vec![0.0, 1.0];
    cuts.extend(sets.iter().flat_map(|s| s.iter().flatten().copied()));
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    cuts.windows(2)
        .filter(|w| w[0] < w[1])
        .map(|w| {
            let mid = (w[0] + w[1]) / 2.0;
            let marks = sets.map(|s| s.iter().any(|&[a, b]| a <= mid && mid <= b));
            let geometry = WkbGeometry {
                points: line.between(w[0], w[1]),
                holes: Vec::new(),
                is_polygon_source: false,
            };
            (marks, [w[0], w[1]], geometry)
        })
        .collect()
}

/// `split_ranged` without the ranges.
pub fn split<const N: usize>(g: WkbGeometry, sets: [&[Range]; N]) -> Vec<([bool; N], WkbGeometry)> {
    split_ranged(g, sets).into_iter().map(|(marks, _, g)| (marks, g)).collect()
}

/// A polyline with the distance along it at each of its points.
pub struct Line<'a> {
    points: &'a [G3857],
    cum: Vec<f64>,
}

impl<'a> Line<'a> {
    /// None for a line of no length, which has no fraction of it to cut at.
    pub fn of(points: &'a [G3857]) -> Option<Line<'a>> {
        let mut cum = Vec::with_capacity(points.len());
        let mut len = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                let q = points[i - 1];
                len += (p[0] - q[0]).hypot(p[1] - q[1]);
            }
            cum.push(len);
        }
        (points.len() >= 2 && len > 0.0).then_some(Line { points, cum })
    }

    pub fn length(&self) -> f64 {
        *self.cum.last().unwrap()
    }

    /// The point at distance `d` along it.
    pub fn at(&self, d: f64) -> G3857 {
        let i = self.cum.partition_point(|&c| c < d).clamp(1, self.points.len() - 1);
        let (p, q) = (self.points[i - 1], self.points[i]);
        let span = self.cum[i] - self.cum[i - 1];
        let t = if span > 0.0 { (d - self.cum[i - 1]) / span } else { 0.0 };
        [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t]
    }

    /// The part of it between two fractions of its length.
    pub fn between(&self, from: f64, to: f64) -> Vec<G3857> {
        let (from, to) = (from * self.length(), to * self.length());
        let mut out = vec![self.at(from)];
        out.extend(self.points.iter().zip(&self.cum).filter(|(_, &c)| c > from && c < to).map(|(p, _)| *p));
        out.push(self.at(to));
        out
    }
}
