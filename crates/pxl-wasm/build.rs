// The module imports its memory rather than owning one: the client hands it
// the core memory (apps/web/src/render/layer/pipeline/core_pool.ts), and the
// module's own stack, data and heap sit in the section reserved for them at
// the front of it (src/arena.rs). Declared no larger than its data needs and
// with no maximum, so the fixed reservation the client makes satisfies it.
fn main() {
    println!("cargo:rustc-link-arg=--import-memory");
}
