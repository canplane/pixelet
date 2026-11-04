// pxl-shader-macro/lib.rs

//! An entry's workgroup written off the constants its code counts by:
//! `#[compute(threads(beam::HIT_LANES), entry_point_name = "hit_lead_cs")]`
//! is `#[spirv(compute(threads(64, 1, 1), entry_point_name = "hit_lead_cs"))]`,
//! each side named in `threads` written as its constant's value, since
//! rust-gpu's `threads` takes literals alone. The constants are
//! pxl-engine's and pxl-shader workgroup.rs's, read here as they are.

#![deny(warnings)]

use proc_macro::{Delimiter, TokenStream, TokenTree};

#[path = "../../src/workgroup.rs"]
mod workgroup;

use pxl_engine::{beam, raster, ray};
use workgroup::{CAST_SIDE, DECOR_CELL_LANES, DECOR_RECORD_LANES};

/// Each constant, by its path as it is written here and in `threads`.
macro_rules! sides {
    ($($side:path),* $(,)?) => { [$((stringify!($side), $side)),*] };
}

/// The constants a side may be named by.
const SIDES: [(&str, u32); 6] = sides![
    beam::HIT_LANES,
    raster::FACES_LANES,
    ray::MISS_LANES,
    CAST_SIDE,
    DECOR_CELL_LANES,
    DECOR_RECORD_LANES,
];

/// rust-gpu's `#[spirv(compute(..))]` over the same arguments, each side
/// `threads` names by a constant of `SIDES` written as its value, a literal
/// as it is, the sides it leaves 1; and the path each is named by asserted
/// that value where the entry is, so it names there the constant it is.
#[proc_macro_attribute]
pub fn compute(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut args = Vec::new();
    let mut held = String::new();
    let mut tokens = attr.into_iter();
    while let Some(t) = tokens.next() {
        let threads = matches!(&t, TokenTree::Ident(i) if i.to_string() == "threads");
        args.push(t.to_string());
        if threads {
            match tokens.next() {
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                    match sides(g.stream()) {
                        Ok(([x, y, z], named)) => {
                            args.push(format!("({x}, {y}, {z})"));
                            for (path, v) in named {
                                held.push_str(&format!("const _: () = assert!({path} == {v});"));
                            }
                        }
                        Err(e) => return refused(&e, item),
                    }
                }
                _ => return refused("`threads` takes its sides in parentheses", item),
            }
        }
    }
    let spirv = format!("{held}#[::spirv_std::spirv(compute({}))]", args.join(" "));
    let mut out: TokenStream = spirv.parse().expect("the attribute's tokens");
    out.extend(item);
    out
}

/// A workgroup's three sides, and each constant's path among them with its
/// value.
type Sides = ([u32; 3], Vec<(String, u32)>);

/// The sides `stream`, the inside of `threads(..)`, gives, by a constant's
/// path or a literal, those it leaves 1.
fn sides(stream: TokenStream) -> Result<Sides, String> {
    let mut named = vec![String::new()];
    for t in stream {
        match t {
            TokenTree::Punct(p) if p.as_char() == ',' => named.push(String::new()),
            t => named
                .last_mut()
                .expect("one side at least")
                .push_str(&t.to_string()),
        }
    }
    named.retain(|n| !n.is_empty());
    if named.is_empty() || named.len() > 3 {
        return Err("`threads` takes 1 to 3 sides".into());
    }
    let (mut out, mut paths) = ([1; 3], Vec::new());
    for (k, n) in named.iter().enumerate() {
        out[k] = match n.parse() {
            Ok(v) => v,
            Err(_) => {
                let v = SIDES
                    .iter()
                    .find(|(path, _)| path.split_whitespace().collect::<String>() == *n)
                    .map(|&(_, v)| v)
                    .ok_or_else(|| {
                        format!(
                            "`{n}` is no workgroup side pxl-shader-macro knows (lib.rs `SIDES`)"
                        )
                    })?;
                paths.push((n.clone(), v));
                v
            }
        };
    }
    Ok((out, paths))
}

/// The compiler's error `why`, the item kept so nothing else fails for it.
fn refused(why: &str, item: TokenStream) -> TokenStream {
    let mut out: TokenStream = format!("::core::compile_error!({why:?});")
        .parse()
        .expect("the error's tokens");
    out.extend(item);
    out
}
