// pxl-geo-bake/terrain/pipeline/overture/between.rs

//! Overture's linear referencing: a rule holds over a `between` range of a
//! segment, or the whole of it. The bake and the deck solve cut the same way.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::adapter::epsg3857::{to_db, G3857};
use serde_json::Value;

use crate::wkb::{wkb_to_geometries, WkbGeometry};

pub type Range = [f64; 2];

/// An unparseable column says nothing.
fn rules(column: Option<&str>) -> Vec<Value> {
    column.and_then(|c| serde_json::from_str::<Value>(c).ok()).and_then(|v| v.as_array().cloned()).unwrap_or_default()
}

fn range_of(rule: &Value) -> Option<Range> {
    let between = rule["between"].as_array().map(|b| (b.first().and_then(Value::as_f64), b.get(1).and_then(Value::as_f64)));
    let [a, b] = match between {
        Some((Some(a), Some(b))) => [a.clamp(0.0, 1.0), b.clamp(0.0, 1.0)],
        _ => [0.0, 1.0],
    };
    (a < b).then_some([a, b])
}

/// A way's length in DB coordinates, as its line runs.
pub fn length_of(geom_wkb: &[u8]) -> f64 {
    wkb_to_geometries(geom_wkb).into_iter().next().and_then(|g| Line::of(&g.points).map(|l| l.length())).unwrap_or(0.0)
}

/// What a ground cell spans of a way `length` long in DB coordinates: a gap
/// in a flag shorter than that shows on no cell, so the ranges either side are one.
pub fn cell_of(length: f64) -> f64 {
    to_db(U_PER_CELL) / length.max(f64::EPSILON)
}

pub fn flag_ranges(flags: Option<&str>, wanted: &[&str], cell: f64) -> Vec<Range> {
    merged(
        rules(flags)
            .iter()
            .filter(|r| r["values"].as_array().is_some_and(|v| v.iter().any(|v| v.as_str().is_some_and(|v| wanted.contains(&v)))))
            .filter_map(range_of)
            .collect(),
        cell,
    )
}

pub fn value_ranges(column: Option<&str>, keep: impl Fn(f64) -> bool) -> Vec<Range> {
    merged(rules(column).iter().filter(|r| r["value"].as_f64().is_some_and(&keep)).filter_map(range_of).collect(), 0.0)
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

/// The first rule that holds at `at`.
pub fn value_at(column: Option<&str>, at: f64) -> Option<f64> {
    rules(column).iter().find(|r| range_of(r).is_some_and(|[a, b]| (a..=b).contains(&at))).and_then(|r| r["value"].as_f64())
}

pub fn text_at(column: Option<&str>, at: f64) -> Option<String> {
    rules(column)
        .iter()
        .find(|r| range_of(r).is_some_and(|[a, b]| (a..=b).contains(&at)))
        .and_then(|r| r["value"].as_str().map(str::to_string))
}

pub fn flag_at(flags: Option<&str>, wanted: &[&str], cell: f64, at: f64) -> bool {
    flag_ranges(flags, wanted, cell).iter().any(|&[a, b]| (a..=b).contains(&at))
}

/// Fractions are of the planar 3857 length where Overture's are geodesic;
/// over one segment the difference is far under a cell.
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

pub fn split<const N: usize>(g: WkbGeometry, sets: [&[Range]; N]) -> Vec<([bool; N], WkbGeometry)> {
    split_ranged(g, sets).into_iter().map(|(marks, _, g)| (marks, g)).collect()
}

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
        let i = self.cum.partition_point(|&c| c < d).clamp(1, self.points.len() - 1);
        let (p, q) = (self.points[i - 1], self.points[i]);
        let span = self.cum[i] - self.cum[i - 1];
        let t = if span > 0.0 { (d - self.cum[i - 1]) / span } else { 0.0 };
        [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t]
    }

    pub fn between(&self, from: f64, to: f64) -> Vec<G3857> {
        let (from, to) = (from * self.length(), to * self.length());
        let mut out = vec![self.at(from)];
        out.extend(self.points.iter().zip(&self.cum).filter(|(_, &c)| c > from && c < to).map(|(p, _)| *p));
        out.push(self.at(to));
        out
    }
}
