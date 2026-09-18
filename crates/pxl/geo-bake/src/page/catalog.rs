// pxl-geo-bake/page/catalog.rs

//! What a published set was baked from, read out of `overture_catalog`.
//!
//! A version directory's name says nothing about when the data under it is
//! from; this is what answers that, and the manifest carries it.

/// The named tables' catalog rows, one key each — `null` where the table has
/// no row, which says "filled by something that did not record it" rather than
/// saying nothing at all.
pub fn sources(client: &mut postgres::Client, tables: &[&str]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for &table in tables {
        let row = client
            .query_opt("SELECT data_date::text AS data_date, total_count FROM overture_catalog WHERE file_name = $1", &[&table])
            .ok()
            .flatten();
        out.insert(
            table.to_string(),
            match row {
                Some(r) => serde_json::json!({ "release": r.get::<_, Option<String>>("data_date"), "row_count": r.get::<_, i64>("total_count") }),
                None => serde_json::Value::Null,
            },
        );
    }
    serde_json::Value::Object(out)
}
