// pxl-tool/spec/bake.rs

use super::ingest::symbol_number;
use xpute_gen::spec::data::{enum_members, table_outputs, DataSpec};
use xpute_gen::spec::ts_enum;

use super::{
    file, json_str, line_comments, mark, pascal, read, plural, u32_of, rs_doc, ts_doc, Node, Output,
};
use crate::root;

/// publication.json5's outputs, by which a spec that names its enums reaches
/// them.
pub(crate) const PUBLICATION: DataSpec = DataSpec {
    json: "spec/spec/bake/publication.json5",
    ts: Some("ts/client/src/abi/publication.spec.ts"),
    rs: Some("rust/spec/src/bake/publication.spec.rs"),
    ..DataSpec::DEFAULT
};

/// publication.json5, written by xpute's generator as a data spec: its
/// `BASE_KIND` enum and `FORMAT` keyed on it among the rest. The tool keeps
/// what is pixelet's own: the object a tile is published as, read and written
/// by `OBJECT`, and the Worker's copy of the layers' names, bundled apart from
/// the client.
pub fn publication_outputs() -> Result<Vec<Output>, String> {
    const JSON: &str = PUBLICATION.json;
    // The enum of the layers, which the spec declares and whose names the
    // Worker's copy takes too.
    const KIND: &str = "BASE_KIND";
    let spec = read(JSON)?;
    let layer = spec.get(KIND).ok_or("publication.json5: no BASE_KIND")?;
    let layers: Vec<&str> = layer.val().arr().unwrap_or_default().iter().filter_map(Node::str).collect();

    // A kind's object in a set: the one place its segments are ordered.
    let object_node = spec.get("OBJECT").ok_or("publication.json5: no OBJECT")?;
    let object = object_node
        .val()
        .str()
        .ok_or("publication.json5: OBJECT is no template")?;
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
            "publication.json5: OBJECT {object:?} is not {{version}}/{{name}}"
        ));
    }
    let object_doc = object_node.doc().unwrap_or_default();
    let sphere_node = spec
        .get("TILE_SPHERE")
        .ok_or("publication.json5: no TILE_SPHERE")?;
    let sphere = sphere_node.val().str().unwrap_or_default();
    if sphere.is_empty()
        || !sphere.bytes().all(|b| b.is_ascii_lowercase())
        || sphere.bytes().all(|b| (b'0'..=b'3').contains(&b))
    {
        return Err(format!(
            "publication.json5: TILE_SPHERE {sphere:?} is no name a tile could have"
        ));
    }
    let sphere_doc = sphere_node.doc().unwrap_or_default();

    let table: Vec<(String, Node)> = spec
        .entries()
        .iter()
        .filter(|(k, _)| k != "OBJECT")
        .cloned()
        .collect();
    let mut outs = table_outputs(&root(), &PUBLICATION, Node::Obj(table), &[])?;

    let mut ts = ts_doc(&object_doc, "");
    ts.extend([
        "export function tile_object(version: string, name: string): string {".to_string(),
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
    ]);
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
            plural(KIND),
            layers
                .iter()
                .map(|k| json_str(k))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ];
    edge.extend(ts_enum(&pascal(KIND), &enum_members(KIND, layer)?));
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
        "spec/spec/bake/place_category.json5",
        "spec/spec/bake/place_theme.json5",
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
        match rows.get(k).and_then(|r| r.get(f)) {
            None => Ok("0".into()),
            Some(c) => u32_of(c).map(|c| format!("{c} << 8 | 0xff")).ok_or(format!("{theme_json}: {k}.{f} is no color")),
        }
    };
    let mut rs = vec![
        "// pxl-spec/bake/place_theme.spec.rs".to_string(),
        "//".into(),
        format!("// {}", mark("spec/spec/bake/place_category.json5 and spec/spec/bake/place_theme.json5", "pxl-tool spec/bake.rs")),
        "//".into(),
    ];
    rs.extend(line_comments(&cat.doc().unwrap_or_default(), ""));
    rs.push("//".into());
    rs.extend(line_comments(
        &theme.doc().unwrap_or_default(),
        "",
    ));
    rs.extend([String::new(), "use crate::color::Rgba;".into()]);
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
        format!("// {}", mark("spec/spec/bake/place_category.json5 and spec/spec/bake/place_theme.json5", "pxl-tool spec/bake.rs")),
        "//".into(),
    ];
    ts.extend(line_comments(
        &theme.doc().unwrap_or_default(),
        "",
    ));
    let fill = |k: &str| -> Result<String, String> {
        match rows.get(k).and_then(|r| r.get("fill")) {
            None => Ok("null".into()),
            Some(c) => u32_of(c).map(|c| format!("{c}")).ok_or(format!("{theme_json}: {k}.fill is no color")),
        }
    };
    for (doc, name, ty, each) in [
        (
            "A category's icon or dot fill, `0xRRGGBB`, null where Carto draws no point for it",
            "FILL",
            "(number | null)",
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
