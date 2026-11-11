// xpute-example/build.rs

// The module is linked as its manifest says (manifest.json, xpute-core
// manifest.rs `link`): it imports its memory, puts its stack first, and places
// its data past the front.

#[path = "src/manifest.spec.rs"]
mod manifest;

fn main() {
    xpute_core::wasm::link(&manifest::MANIFEST.link());
}
