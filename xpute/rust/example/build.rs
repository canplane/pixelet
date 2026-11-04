// xpute-example/build.rs

// The module linked as its machine says (machine.json, xpute-guest machine.rs
// `link`): its memory imported, its stack first, its data past the front.

#[path = "machine.spec.rs"]
mod machine;

fn main() {
    xpute_guest::wasm::link(&machine::MACHINE.link());
}
