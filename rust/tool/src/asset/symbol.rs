// pxl-tool/asset/symbol.rs

//! Carto's place symbols, each SVG under `asset/raw/place/symbol/` named by the path Carto
//! gives it (`symbols/…`), which is how the places' theme names it. For the marker atlas they
//! become the masks it tints, each rasterized once to its coverage, a byte a pixel, in a
//! square of `SYMBOL_PX`, laid end to end in one file the spec crate includes; for the UI,
//! their outlines.

use resvg::tiny_skia::Pixmap;
use resvg::tiny_skia::Transform;
use resvg::usvg::FillRule;
use resvg::usvg::Group;
use resvg::usvg::Node;
use resvg::usvg::Tree;
use std::path::Path;

use xpute_gen::spec::data::{table_outputs, DataSpec};
use xpute_gen::spec::{description, Node as SpecNode, Output};

/// What the symbols are read from, and the files written from them, which
/// `spec` leaves to the asset tool.
const SOURCE: &str = "asset/raw/place/symbol/";
pub const RS: &str = "rust/spec/src/asset/symbol.spec.rs";
pub const TS: &str = "ts/client/src/ui/feature/search/place-symbol.ts";

/// The side of a symbol's mask in device pixels: Carto's side at the atlas's device pixels a
/// pixel.
pub(crate) const SYMBOL_PX: u32 = CARTO_PX * SYMBOL_RATIO;

/// Carto's symbols' side, in pixels.
const CARTO_PX: u32 = 14;

/// The device pixels a pixel at which the symbols are rasterized, written into the spec.
/// pxl-render look/marker.rs draws the atlas at this scale, so a display finer than this
/// shows a marker scaled up.
const SYMBOL_RATIO: u32 = 2;

pub struct Symbol {
    /// As Carto gives it.
    pub path: String,
    pub mask: Vec<u8>,
    /// Its own box, and each shape in it as SVG path data and whether it
    /// fills even-odd.
    pub size: [f32; 2],
    pub(crate) shapes: Vec<(String, bool)>,
}

/// Each symbol, sorted by path.
pub fn symbols(dir: &Path) -> Result<Vec<Symbol>, String> {
    let mut paths = Vec::new();
    walk(dir, dir, &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .map(|rel| {
            let svg = std::fs::read(dir.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
            let tree = Tree::from_data(&svg, &resvg::usvg::Options::default())
                .map_err(|e| format!("{rel}: {e}"))?;
            let mut shapes = Vec::new();
            outline(tree.root(), &mut shapes);
            Ok(Symbol {
                path: format!("symbols/{rel}"),
                mask: mask(&tree).map_err(|e| format!("{rel}: {e}"))?,
                size: [tree.size().width(), tree.size().height()],
                shapes,
            })
        })
        .collect()
}

/// Every filled path under `group`, placed where it is drawn.
fn outline(group: &Group, out: &mut Vec<(String, bool)>) {
    use resvg::tiny_skia::PathSegment;
    for node in group.children() {
        match node {
            Node::Group(g) => outline(g, out),
            Node::Path(p) => {
                let Some(fill) = p.fill() else { continue };
                let Some(data) = p.data().clone().transform(p.abs_transform()) else {
                    continue;
                };
                let n = |v: f32| format!("{}", (v * 1000.0).round_ties_even() / 1000.0);
                let d: Vec<String> = data
                    .segments()
                    .map(|s| match s {
                        PathSegment::MoveTo(a) => format!("M{} {}", n(a.x), n(a.y)),
                        PathSegment::LineTo(a) => format!("L{} {}", n(a.x), n(a.y)),
                        PathSegment::QuadTo(a, b) => {
                            format!("Q{} {} {} {}", n(a.x), n(a.y), n(b.x), n(b.y))
                        }
                        PathSegment::CubicTo(a, b, c) => format!(
                            "C{} {} {} {} {} {}",
                            n(a.x),
                            n(a.y),
                            n(b.x),
                            n(b.y),
                            n(c.x),
                            n(c.y)
                        ),
                        PathSegment::Close => "Z".into(),
                    })
                    .collect();
                out.push((d.join(""), fill.rule() == FillRule::EvenOdd));
            }
            _ => {}
        }
    }
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, base, out)?;
        } else if path.extension().is_some_and(|e| e == "svg") {
            out.push(
                path.strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

/// The symbol's coverage, scaled from its own box to fill the square.
fn mask(tree: &Tree) -> Result<Vec<u8>, String> {
    let size = tree.size();
    let mut pixmap = Pixmap::new(SYMBOL_PX, SYMBOL_PX).ok_or("an empty square")?;
    let scale = SYMBOL_PX as f32 / size.width().max(size.height());
    let transform = Transform::from_scale(scale, scale).post_translate(
        (SYMBOL_PX as f32 - size.width() * scale) / 2.0,
        (SYMBOL_PX as f32 - size.height() * scale) / 2.0,
    );
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(pixmap.pixels().iter().map(|p| p.alpha()).collect())
}

/// The symbols' files, from a data spec built here and written by xpute's
/// generator: their side, their scale and their count on both sides, then in
/// Rust the masks the spec crate includes, and in TypeScript the UI's
/// outlines, each symbol's box and shapes, by its number.
pub fn outputs(root: &Path, symbols: &[Symbol]) -> Result<Vec<Output>, String> {
    let entry = |doc: &[&str], ty: &str, n: usize| {
        SpecNode::Obj(vec![
            ("$description".into(), description(doc)),
            ("$type".into(), SpecNode::Str(ty.into())),
            ("$value".into(), SpecNode::Num(n as f64)),
        ])
    };
    let table = SpecNode::Obj(vec![
        (
            "$description".into(),
            description(&[
                "Carto's place symbols (openstreetmap-carto `symbols/`, CC0), symbol n the (n - 1)th in",
                "spec/spec/asset/ingest.json5's `symbols` order: in Rust each a mask of its coverage, a",
                "byte a pixel, `SYMBOL_PX` a side, row by row, in `symbol.mask`; in TypeScript its",
                "outline at `PLACE_SYMBOLS[n - 1]` (place_theme.spec.ts `SYMBOL`), its box and each",
                "shape's path data and whether it fills even-odd.",
            ]),
        ),
        ("SYMBOL_PX".into(), entry(&["A mask's side, in device pixels."], "u32", SYMBOL_PX as usize)),
        (
            "SYMBOL_RATIO".into(),
            entry(&["The device pixels a pixel the masks are rasterized at."], "u32", SYMBOL_RATIO as usize),
        ),
        ("SYMBOL_CNT".into(), entry(&["How many symbols there are."], "usize", symbols.len())),
    ]);
    let d = DataSpec {
        json: SOURCE,
        ts: Some(TS),
        rs: Some(RS),
        ..DataSpec::DEFAULT
    };
    let mut outs = table_outputs(root, &d, table, &[])?;
    let rs = "\npub static SYMBOL_MASKS: &[u8] = include_bytes!(\"symbol.mask\");\n".to_string();
    let mut ts = String::from(
        "\nexport interface PlaceSymbol {\n  box: readonly [number, number];\n  shapes: readonly (readonly [string, boolean])[];\n}\n\nexport const PLACE_SYMBOLS: readonly PlaceSymbol[] = [\n",
    );
    for s in symbols {
        let shapes: Vec<String> = s
            .shapes
            .iter()
            .map(|(d, even)| format!("[{d:?}, {even}]"))
            .collect();
        ts.push_str(&format!(
            "  {{ box: [{}, {}], shapes: [{}] }},\n",
            s.size[0],
            s.size[1],
            shapes.join(", ")
        ));
    }
    ts.push_str("];\n");
    for o in &mut outs {
        o.text.push_str(if o.path == RS { &rs } else { &ts });
    }
    Ok(outs)
}
