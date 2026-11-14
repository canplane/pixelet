# xpute-gen

Some facts are held by more than one language: a table of constants a guest
and its host both read, an enum whose numbers cross the link, a layout of
words a C++ host decodes. xpute-gen keeps each such fact in one spec and
writes it into every language that holds it. Generation is for such a fact
alone, one that crosses languages and that a spec owns; a number only one
language reads is written by hand where it is used.

Its forms are the established generators': wit-bindgen's for Rust bindings,
jco's for TypeScript and prost's for data specs. It departs from them where
our conditions differ, and only there: an enum is its members' number
constants rather than jco's strings, because the number is what crosses the
link and a table of names would ship in the bundle.

## One translation

The generator is a one-to-one translation. Each concept of the spec becomes
the same concept in each language, written in that language's own form: the
generator works with the spec's own concepts alone, and holds no concept of
the program it writes for and no special case. The languages agree because
none of them owns the fact; each holds a translation of the one spec. Rust,
TypeScript and C++ are written alike, and differ only where a language forces
it, and each concept has one writer that every generator calls (`rs_enum`,
`ts_enum`, `cpp_enum`). A name is one spelling in every language: functions,
fields and variables in snake_case, types in Pascal, constants in SCREAMING. A
type's generated helper is its verb, an underscore and the type's own name
(`of_Tool`, `store_LonLat`).

A value goes as it is: a number keeps its value, a string its text, and a
string a `$type` names as an enum's member is that member. A `$type` the
generator does not know is refused. An enum that is a list is numbered by its
members' places, or, where every record carries one, each by its `$number`;
an enum that is a table gives each member a number of its own. A number goes
at its own width, and a list of fixed-size numbers is packed at its
element's. A table with `"$packed": true` is a layout of words: each field's
width is an element or `[count, element]`, and the generator writes where
each field begins.

## Only what the compiler uses

What is written is what a compiler or a bundler uses, and no more: a key, a
name or a lookup that no reader takes is not written. In Rust an enum is an
enum, with `of`, a number to its member, and `ALL` and `name()`. In
TypeScript each member is a constant of its own (`export const TOOL_ITEM =
1`), the enum's type is the union of them, and `of_Tool` is Rust's `of`,
comparing a number with the members' constants; a reader imports what it
uses by name, so a bundle's tree shaking keeps the numbers and drops every
name. A list of records whose names are an enum's members gives each
TypeScript row its member's `number` and no `name`. Where a reader shows a
group's names, the spec marks it `"$names": true`. A spec reaches another's
enum through `$uses`, and the other spec's output is imported, never copied.

## Writing and checking

The generated files are committed beside the code that uses them. From
`xpute/`, `cargo spec` checks that every one is what its spec writes, and
`cargo spec --write` writes them. Each file goes through the formatter its
tree checks it with, and the result passes the lints and the type check that
tree runs.

The link's bindings are written from WIT by xpute-link-gen
([`link/gen`](../link/gen/README.md)), through this crate's writers and by
these rules.
