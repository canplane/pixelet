// pxl-render/look/cloud.test.rs
//
// The lattice hash is written twice, here for the kernel's cumulus and in
// noise.wgsl for the sheets' field, and both must draw the same cells: the
// WGSL's body carries this one's numbers in this one's order.

/// The numbers in function `name`'s body in `source`, as written less a
/// digit separator or an unsigned suffix: what the two languages spell
/// alike once those go.
fn numbers(source: &str, name: &str) -> Vec<String> {
    let at = source.find(&format!("fn {name}(")).unwrap_or_else(|| panic!("no fn {name}"));
    let body = &source[at..];
    let body = &body[body.find('{').expect("a body")..body.find("\n}").expect("a closed body")];
    let bytes = body.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'.';
    let mut out = Vec::new();
    let mut k = 0;
    while k < bytes.len() {
        if bytes[k].is_ascii_digit() && (k == 0 || !word(bytes[k - 1])) {
            let start = k;
            while k < bytes.len() && word(bytes[k]) {
                k += 1;
            }
            let n = body[start..k].replace('_', "").to_ascii_lowercase();
            out.push(n.strip_suffix('u').map_or(n.clone(), str::to_string));
        } else {
            k += 1;
        }
    }
    out
}

#[test]
fn the_shaders_hash_is_this_one() {
    let rust = numbers(include_str!("cloud.rs"), "lattice_hash");
    let wgsl = numbers(include_str!("../frame/shader/noise.wgsl"), "lattice_hash");
    assert_eq!(rust.len(), 10, "the multipliers, shifts and scale: {rust:?}");
    assert_eq!(rust, wgsl);
}
