// pxl-geo-bake/page/catalog.rs

/// `null` where a table has no catalog row.
pub fn sources(client: &mut postgres::Client, tables: &[&str]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for &table in tables {
        let row = client
            .query_opt(
                &format!("SELECT data_date::text AS data_date, total_count FROM {} WHERE file_name = $1", pxl_geo_spec::table::CATALOG),
                &[&table],
            )
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
