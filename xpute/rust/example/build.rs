// xpute-example/build.rs

// Linked as a module guest is, with the link numbers `program!` assumes.

fn main() {
    xpute_guest::wasm::link(&xpute_guest::wasm::LinkConfig::DEFAULT);
}
