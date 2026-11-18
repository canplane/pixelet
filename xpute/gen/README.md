# xpute-gen

Some facts are held by more than one language: a table of constants a guest
and its host both read, an enum whose numbers cross the link, a layout of
words a C++ host decodes. xpute-gen keeps each such fact in one spec and
writes it into every language that holds it. Generation is for such a fact
alone, one that crosses languages and that a spec owns; a number only one
language reads is written by hand where it is used.

Its forms are the established generators': wit-bindgen's for Rust bindings,
jco's for TypeScript and prost's for data specs, with one departure: an enum
is its members' number constants where jco has strings, since the number is
what crosses the link.

## One translation

The generator is a one-to-one translation. Each concept of the spec becomes
the same concept in each language, written in that language's own form, and
each concept has one writer that every generator calls (`rs_enum`, `ts_enum`,
`cpp_enum`). A value keeps its spec's spelling in TypeScript and C++, and Rust
recases it to its own convention: a field in snake_case, a constant in
SCREAMING. An enum's member is Pascal in Rust, snake_case in C++ and a
SCREAMING constant under its enum's name in TypeScript (`TOOL_ITEM`). A type's
generated helper is an associated function in Rust (`Tool::of`), and in
TypeScript and C++ its verb, an underscore and the type's own name (`of_Tool`,
`store_LonLat`).

A spec is written in the Design Tokens Community Group's
[format](https://www.designtokens.org/tr/drafts/format/), in JSON5 so that a
number written in hex is a number: a value is a token's `$value`, typed by the
`$type` it carries or its group's, and explained by its `$description`. It
departs from DTCG in its scalar types, which are ours (`u32`, `f32`, `str`)
where DTCG has none, and in its own marks, which go under a node's
`$extensions`, in `xpute`. The generator knows basic types alone (numbers,
strings, booleans, enums, lists and tables) and no domain type: a color is a
number.

A value goes as it is: a number keeps its value, a string its text, and a
string a `$type` names as an enum's member is that member, which a C++
constant alone cannot be. A value may be DTCG's reference to another token of
its spec, `"{group.token}"`, and is then that token's value. A `$type` the
generator does not know is refused, a Rust path (`a::B`) apart, and so is a
number with no `$type`, its own or its group's, or with a type no number has,
or one its integer `$type` does not hold, not whole or past its width, a
packed table's words a `usize`'s. An enum that is a list is numbered by its
members' places, or, where every record carries one, each by its `number`; an
enum that is a table gives each member a number of its own. A number goes at
its own width, and a list of fixed-size numbers is an array of its element's
type in Rust and C++, and a plain array in TypeScript. A table marked
`"packed": true` is a layout of words: each field's width is an element or
`[count, element]`, and the generator writes where each field begins.

## Only what the compiler uses

What is written is what a compiler or a bundler can drop where no reader takes
it. In Rust an enum is an enum, with `of`, a number to its member, and `ALL`
and `name()`. In TypeScript each member is a constant of its own (`export
const TOOL_ITEM = 1`), the enum's type is the union of them, `of_Tool` is
Rust's `of`, comparing a number with the members' constants, and `name_Tool`
its `name()`; a reader imports what it uses by name, so a bundle's tree
shaking keeps the numbers a reader takes and drops the rest. A list of records
whose names are an enum's members gives each
TypeScript row its member's `number` and no `name`. Where a reader shows a
group's names, the spec marks it `"names": true`. A spec reaches another's
enum through its `uses`, and the other spec's output is imported, never copied.

## Writing and checking

The generated files are committed beside the code that uses them. From
`xpute/`, `cargo spec` checks that every one is what its spec writes, and
`cargo spec --write` writes them. A Rust file goes through rustfmt, and a
TypeScript one through the prettier of the pnpm tree it lies in; a C++ header
is written as generated. xpute's checks ([CONTRIBUTING.md](../CONTRIBUTING.md))
hold a generated file to the lints and the type check of the code beside it.

The link's bindings are written from WIT by xpute-link-gen
([`link/gen`](../link/gen/README.md)), through this crate's writers and by
these rules.
