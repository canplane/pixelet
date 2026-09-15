// pxl_core/module/world/search/repository.test.rs

use super::*;
use crate::module::world::geocode::label::assemble_label;

#[test]
fn a_search_response_is_read_into_its_table_with_each_result_s_context_and_names() {
    let body = r#"[
        {"gers_id":"a","kind":"division","name":"서울","category":null,"common":{"en":"Seoul","ko":"서울"},"context":[{"name":"대한민국","common":{"en":"South Korea"}}],"lon":127.0,"lat":37.5},
        {"gers_id":"b","kind":"poi","name":"Cafe \"Q\"","category":"cafe","context":[{"name":"서울"},{"name":"강남구","common":{"en":"Gangnam-gu"}}],"lon":127.1,"lat":37.4},
        {"gers_id":"broken","kind":"poi","name":"no point"}
    ]"#;
    let mut t = SearchTable::new();
    assert!(parse_search(&mut t, body));
    let r = t.results();
    assert_eq!(r.len(), 2, "a result without its point is left out");
    assert_eq!((r[0].gers_id.as_str(), r[0].kind), ("a", SearchKind::Division));
    assert_eq!(t.parts.name_in(r[0].common, &r[0].name, "en"), "Seoul");
    assert_eq!(t.parts.name_in(r[0].common, &r[0].name, "ja"), "서울", "a language with no translation falls back to the name");
    assert_eq!((r[1].name.as_str(), r[1].category.as_str(), r[1].lon), ("Cafe \"Q\"", "cafe", 127.1));
    assert_eq!(assemble_label(&t.parts, r[1].context, "en-US").to_string(), "Gangnam-gu, 서울");
    assert_eq!(assemble_label(&t.parts, r[1].context, "ko").to_string(), "서울 강남구");

    assert!(!parse_search(&mut t, "{}"), "not an array");
    assert!(t.results().is_empty());
}
