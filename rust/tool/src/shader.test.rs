// pxl-tool/shader.test.rs

use super::*;

#[test]
fn comments_ignored() {
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
fn toml_value() {
    let bare = meaning("a.toml", "[a]\nb = 1\nc = \"#d\"\n").unwrap();
    let said = meaning("a.toml", "# The table.\n[a]\nc = \"#d\"  # kept\n\nb = 1\n").unwrap();
    let moved = meaning("a.toml", "[a]\nb = 2\nc = \"#d\"\n").unwrap();
    assert_eq!(bare, said);
    assert_ne!(bare, moved);
}

#[test]
fn workspace_inheritance() {
    let base = "[workspace]\nmembers = [\"a\"]\n[workspace.package]\nedition = \"2021\"\n[profile.dev]\nopt-level = 0\n";
    let profiled = "[workspace]\nmembers = [\"a\", \"b\"]\n[workspace.package]\nedition = \"2021\"\n[profile.dev]\nopt-level = 1\n";
    let edition = "[workspace]\nmembers = [\"a\"]\n[workspace.package]\nedition = \"2024\"\n";
    let f = "rust/Cargo.toml";
    assert_eq!(meaning(f, base).unwrap(), meaning(f, profiled).unwrap());
    assert_ne!(meaning(f, base).unwrap(), meaning(f, edition).unwrap());
}
