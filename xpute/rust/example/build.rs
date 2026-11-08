// xpute-example/build.rs

// Links the module as its machine says (machine.json, xpute-core machine.rs
// `link`): it imports its memory, puts its stack first, and places its data
// past the front.

#[path = "machine.spec.rs"]
mod machine;

fn main() {
    xpute_core::wasm::link(&machine::MACHINE.link());
}
