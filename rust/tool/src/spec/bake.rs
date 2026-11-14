// pxl-tool/spec/bake.rs

use super::ingest::symbol_number;
use xpute_gen::spec::data::{table_outputs, DataSpec};

use super::{
    file, json_str, line_comments, mark, read, plural, rs_const, rs_doc, rs_member, ts_doc, Node, Output,
};
use crate::root;

/// publication.json, written by xpute's generator as a data spec, with
/// `BASE_LAYER`, the enum of `FORMAT`'s kinds in their order, its names
/// `BASE_LAYERS`, beside it. The tool keeps what is pixelet's own: the object a
/// tile is published as, read and written by `OBJECT`, and the Worker's copy of
/// the kinds, bundled apart from the client.
pub fn publication_outputs() -> Result<Vec<Output>, String> {
    const JSON: &str = "spec/spec/bake/publication.json";
    // The enum of the layers, whose names the Worker's copy takes too.
    const LAYER: &str = "BASE_LAYER";
    let spec = read(JSON)?;
    let format = spec.get("FORMAT").ok_or("publication.json: no FORMAT")?;
    let layers: Vec<&str> = format.named().map(|(k, _)| k).collect();

    // A kind's object in a set: the one place its segments are ordered.
    let object_node = spec.get("OBJECT").ok_or("publication.json: no OBJECT")?;
    let object = object_node
        .val()
        .str()
        .ok_or("publication.json: OBJECT is no template")?;
    let segments: Vec<&str> = object
        .split('/')
        .map(|s| {
            s.strip_prefix('{')
                .and_then(|s| s.strip_suffix('}'))
                .unwrap_or_default()
        })
        .collect();
    // A version is a folder its tiles go up in whole (script/publish.py), so it comes first.
    if segments != ["version", "name"] {
        return Err(format!(
            "publication.json: OBJECT {object:?} is not {{version}}/{{name}}"
        ));
    }
    let object_doc = object_node.lines_at("$doc").unwrap_or_default();
    let sphere_node = spec
        .get("TILE_SPHERE")
        .ok_or("publication.json: no TILE_SPHERE")?;
    let sphere = sphere_node.val().str().unwrap_or_default();
    if sphere.is_empty()
        || !sphere.bytes().all(|b| b.is_ascii_lowercase())
        || sphere.bytes().all(|b| (b'0'..=b'3').contains(&b))
    {
        return Err(format!(
            "publication.json: TILE_SPHERE {sphere:?} is no name a tile could have"
        ));
    }
    let sphere_doc = sphere_node.lines_at("$doc").unwrap_or_default();

    let mut table: Vec<(String, Node)> = spec
        .entries()
        .iter()
        .filter(|(k, _)| k != "OBJECT")
        .cloned()
        .collect();
    table.push((
        LAYER.into(),
        Node::Obj(vec![
            ("$type".into(), Node::Str("enum".into())),
            ("$names".into(), Node::Bool(true)),
            ("$repr".into(), Node::Str("u32".into())),
            (
                "$doc".into(),
                Node::Arr(vec![Node::Str(
                    "The base layers a version is published for: the kinds of `FORMAT`, numbered in their order.".into(),
                )]),
            ),
            (
                "$val".into(),
                Node::Arr(layers.iter().map(|k| Node::Str(k.to_string())).collect()),
            ),
        ]),
    ));
    let d = DataSpec {
        json: JSON,
        ts: Some("ts/client/src/service/origin/publication.spec.ts"),
        rs: Some("rust/spec/src/bake/publication.spec.rs"),
        ..DataSpec::DEFAULT
    };
    let mut outs = table_outputs(&root(), &d, Node::Obj(table))?;

    let mut ts = ts_doc(&object_doc, "");
    ts.extend([
        "export function tileObject(version: string, name: string): string {".to_string(),
        format!("  return `{}`;", object.replace('{', "${")),
        "}".into(),
    ]);
    let mut rs = rs_doc(&object_doc, "", "///");
    rs.extend([
        "/// Written into `w`, so a key is made where it is wanted, not allocated.".to_string(),
        "pub fn tile_object(w: &mut impl core::fmt::Write, version: &str, name: impl core::fmt::Display) -> core::fmt::Result {".to_string(),
        format!("    write!(w, {})", json_str(object)),
        "}".into(),
        String::new(),
        "/// `tile_object`'s version and name, none for what it never writes.".into(),
        "pub fn tile_object_parts(object: &str) -> Option<(&str, &str)> {".into(),
        "    let mut segment = object.split('/');".into(),
        format!(
            "    let ({}) = ({});",
            segments.join(", "),
            segments.iter().map(|_| "segment.next()?").collect::<Vec<_>>().join(", ")
        ),
        "    segment.next().is_none().then_some((version, name))".into(),
        "}".into(),
        String::new(),
        "impl BaseLayer {".into(),
        "    /// The format the layer is baked and read in, `FORMAT[layer]`.".into(),
        "    pub const fn format(self) -> u32 {".into(),
        "        match self {".into(),
    ]);
    rs.extend(
        layers
            .iter()
            .map(|k| format!("            BaseLayer::{} => format::{},", rs_member(k), rs_const(k))),
    );
    rs.extend(["        }", "    }", "}"].map(String::from));
    for o in &mut outs {
        let tail = if o.path.ends_with(".rs") { &rs } else { &ts };
        o.text = format!("{}\n{}", o.text, file(tail));
    }

    // The Worker's own: it is bundled for the Workers runtime, apart from the
    // client, so it reads a copy beside it written from the same JSON.
    let mut edge = vec![
        "// @pxl/edge/publication.spec.ts".to_string(),
        "//".into(),
        format!("// {}", mark(JSON, "pxl-tool spec/bake.rs")),
        "".into(),
        "/** The base layers, each a kind a baked page is published under. */".into(),
        format!(
            "export const {} = [{}] as const;",
            plural(LAYER),
            layers
                .iter()
                .map(|k| json_str(k))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ];
    // A version is any one segment the declaration names. A name is digits from 0 to 3, one
    // digit for each zoom (pxl-spec space/hierarchy/morton.rs).
    let pattern = object
        .replace("{version}", "[^/]+")
        .replace("{name}", "[0-3]+")
        .replace('/', "\\/");
    edge.push("".into());
    edge.extend(ts_doc(
        &[format!(
            "A tile's object under its kind, `{object}`, as a pattern."
        )],
        "",
    ));
    edge.push(format!("export const TILE_OBJECT = String.raw`{pattern}`;"));
    edge.push("".into());
    edge.extend(ts_doc(&sphere_doc, ""));
    edge.push(format!("export const TILE_SPHERE = {};", json_str(sphere)));
    outs.push(Output::new(
        "ts/server/edge/publication.spec.ts",
        file(&edge),
    ));
    Ok(outs)
}

// The Rust reads a place's look by its category number: for each category, in lists the
// core indexes, what Carto draws for it, with 0 for a zoom at which it never shows.

pub fn place_theme_outputs() -> Result<Vec<Output>, String> {
    let (cat_json, theme_json) = (
        "spec/spec/bake/place_category.json",
        "spec/spec/bake/place_theme.json",
    );
    let (cat, theme) = (read(cat_json)?, read(theme_json)?);
    let categories: Vec<&str> = cat
        .get("CATEGORIES")
        .and_then(|c| c.arr())
        .ok_or(format!("{cat_json}: no CATEGORIES"))?
        .iter()
        .filter_map(|k| k.str())
        .collect();
    // A category is a half-word on the wire, and 0 is none.
    if categories.len() >= u16::MAX as usize {
        return Err(format!(
            "{cat_json}: {} categories do not fit a half-word beside none",
            categories.len()
        ));
    }
    let rows = theme
        .get("THEME")
        .ok_or(format!("{theme_json}: no THEME"))?;
    if let Some((k, _)) = rows.named().find(|(k, _)| !categories.contains(k)) {
        return Err(format!("{theme_json}: {k} is no category of {cat_json}"));
    }
    let zoom = |k: &str, f: &str| -> Result<String, String> {
        match rows.get(k).and_then(|r| r.get(f)) {
            None => Ok("0".into()),
            Some(z) => z
                .num()
                .filter(|z| (1.0..=31.0).contains(z) && z.fract() == 0.0)
                .map(|z| format!("{z}"))
                .ok_or(format!("{theme_json}: {k}.{f} is not a zoom")),
        }
    };
    let color = |k: &str, f: &str| -> Result<String, String> {
        match rows.get(k).and_then(|r| r.str_at(f)) {
            None => Ok("0".into()),
            Some(c) => Ok(format!("rgb({}) << 8 | 0xff", json_str(c))),
        }
    };
    let mut rs = vec![
        "// pxl-spec/bake/place_theme.spec.rs".to_string(),
        "//".into(),
        format!("// {}", mark("spec/spec/bake/place_category.json and spec/spec/bake/place_theme.json", "pxl-tool spec/bake.rs")),
        "//".into(),
    ];
    rs.extend(line_comments(&cat.lines_at("$doc").unwrap_or_default(), ""));
    rs.push("//".into());
    rs.extend(line_comments(
        &theme.lines_at("$doc").unwrap_or_default(),
        "",
    ));
    rs.extend([String::new(), "use crate::color::{rgb, Rgba};".into()]);
    let mut list = |doc: &str,
                    ty: &str,
                    name: &str,
                    each: &dyn Fn(&str) -> Result<String, String>|
     -> Result<(), String> {
        rs.extend([
            String::new(),
            format!("/// {doc}, category k at `{name}[k - 1]`."),
            format!("pub const {name}: [{ty}; {}] = [", categories.len()),
        ]);
        for k in &categories {
            rs.push(format!("    {},", each(k)?));
        }
        rs.push("];".into());
        Ok(())
    };
    list(
        "Each category's Overture name",
        "&str",
        "CATEGORIES",
        &|k| Ok(json_str(k)),
    )?;
    list(
        "The zoom from which Carto draws a category as a dot, 0 for never",
        "u8",
        "DOT_ZOOM",
        &|k| zoom(k, "dot"),
    )?;
    list(
        "The zoom from which Carto draws a category's icon, 0 for never",
        "u8",
        "ICON_ZOOM",
        &|k| zoom(k, "icon"),
    )?;
    list(
        "The zoom from which Carto names a category, 0 for never",
        "u8",
        "LABEL_ZOOM",
        &|k| zoom(k, "label"),
    )?;
    list(
        "A category's icon or dot fill",
        "Rgba",
        "FILL",
        &|k| color(k, "fill"),
    )?;
    list(
        "A category's label fill, its ink",
        "Rgba",
        "INK",
        &|k| color(k, "text"),
    )?;
    let symbol = |k: &str| -> Result<String, String> {
        match rows.get(k).and_then(|r| r.str_at("symbol")) {
            None | Some("") => Ok("0".into()),
            Some(path) => symbol_number(path).map(|n| n.to_string()),
        }
    };
    list(
        "A category's icon, its symbol's number (asset/symbol.spec.rs), 0 for none",
        "u16",
        "SYMBOL",
        &symbol,
    )?;
    // The UI's table holds what it draws a search result's place as, by the category's number.
    let mut ts = vec![
        "// @pxl/client/ui/feature/search/place_theme.spec.ts".to_string(),
        "//".into(),
        format!("// {}", mark("spec/spec/bake/place_category.json and spec/spec/bake/place_theme.json", "pxl-tool spec/bake.rs")),
        "//".into(),
    ];
    ts.extend(line_comments(
        &theme.lines_at("$doc").unwrap_or_default(),
        "",
    ));
    let fill = |k: &str| -> Result<String, String> {
        match rows.get(k).and_then(|r| r.str_at("fill")) {
            None => Ok(json_str("")),
            Some(c) => Ok(json_str(c)),
        }
    };
    for (doc, name, ty, each) in [
        (
            "A category's icon or dot fill as `#rrggbb`, \"\" where Carto draws no point for it",
            "FILL",
            "string",
            &fill as &dyn Fn(&str) -> Result<String, String>,
        ),
        (
            "A category's icon, its symbol's number (place-symbol.ts), 0 for none",
            "SYMBOL",
            "number",
            &symbol,
        ),
    ] {
        ts.extend([
            String::new(),
            format!("/** {doc}, category k at `{name}[k - 1]`. */"),
            format!("export const {name}: readonly {ty}[] = ["),
        ]);
        for k in &categories {
            ts.push(format!("  {},", each(k)?));
        }
        ts.push("];".into());
    }
    Ok(vec![
        Output::new("rust/spec/src/bake/place_theme.spec.rs", file(&rs)),
        Output::new(
            "ts/client/src/ui/feature/search/place_theme.spec.ts",
            file(&ts),
        ),
    ])
}
