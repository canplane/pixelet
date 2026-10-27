// pxl-tool/shader.test.rs

use super::*;

#[test]
fn a_comment_or_a_doc_is_no_part_of_what_a_rust_file_means() {
    let bare = meaning("a.rs", "pub fn f(x: u32) -> u32 { x + 1 }").unwrap();
    let said = meaning(
        "a.rs",
        "//! The module.\n\n/// One more.\n#[doc = \"again\"]\npub fn f(x: u32) -> u32 {\n    // the step\n    x + /* one */ 1\n}\n",
    )
    .unwrap();
    assert_eq!(bare, said);
}

#[test]
fn a_token_is() {
    let one = meaning("a.rs", "pub fn f(x: u32) -> u32 { x + 1 }").unwrap();
    let two = meaning("a.rs", "pub fn f(x: u32) -> u32 { x + 2 }").unwrap();
    let joined = meaning("a.rs", "pub fn f(x: u32) -> u32 { x+=1 }").unwrap();
    let apart = meaning("a.rs", "pub fn f(x: u32) -> u32 { x+ =1 }").unwrap();
    assert_ne!(one, two);
    assert_ne!(joined, apart);
}

#[test]
fn a_toml_file_means_its_value() {
    let bare = meaning("a.toml", "[a]\nb = 1\nc = \"#d\"\n").unwrap();
    let said = meaning("a.toml", "# The table.\n[a]\nc = \"#d\"  # kept\n\nb = 1\n").unwrap();
    let moved = meaning("a.toml", "[a]\nb = 2\nc = \"#d\"\n").unwrap();
    assert_eq!(bare, said);
    assert_ne!(bare, moved);
}
