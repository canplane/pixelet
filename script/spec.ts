#!/usr/bin/env -S deno run -A
// The specs are JSON, and the TypeScript and the Rust that carry them are
// written from it: neither language is where a number or a name is decided.
// Run as `deno task spec` to write every generated file, and as `deno task
// check:spec` (part of `deno task check`) to fail where one on disk is not
// what its JSON writes.
//
// A generated file is imported as any module is — the client's bundler
// does not take a JSON import from outside its root — and is never edited
// by hand.

import { dirname, join } from "@std/path";

const root = dirname(dirname(new URL(import.meta.url).pathname));

interface Output {
  path: string;
  text: string;
}

// ---- the command table ----

interface CmdField {
  name: string;
  type: string;
  optional?: true;
  list?: true;
  doc?: string[];
}
interface CmdEntry {
  name: string;
  value: number;
  doc?: string[];
  packet?: CmdField[];
  signal?: CmdField[];
}
interface CmdSection {
  section: string;
  doc?: string[];
}
interface CmdEnum {
  name: string;
  doc?: string[];
  format: "hex2" | "hex4" | "dec";
  default?: string;
  unknown?: string;
  entries: (CmdEntry | CmdSection)[];
}
interface CmdExtern {
  name: string;
  rs: string;
  ts?: string;
  repr?: "u8" | "u32";
  unknown?: string;
  doc?: string[];
}
interface CmdRecord {
  name: string;
  doc?: string[];
  fields: CmdField[];
}
interface CmdSpec {
  doc: string[];
  extern: CmdExtern[];
  enums: CmdEnum[];
  records: CmdRecord[];
}

/** The numbers a field can be, by the width the wire gives them. */
const CMD_NUMBERS = new Set(["u8", "u32", "i32", "u64", "f64"]);

/** What a field's type is: a number or a bool, a string, one of the spec's
 * enums or another spec's, or one of its records. */
function cmdKind(spec: CmdSpec, type: string): "number" | "bool" | "str" | "enum" | "extern" | "record" {
  if (CMD_NUMBERS.has(type)) return "number";
  if (type === "bool" || type === "str") return type;
  if (spec.enums.some((e) => e.name === type)) return "enum";
  if (spec.extern.some((e) => e.name === type)) return "extern";
  if (spec.records.some((r) => r.name === type)) return "record";
  throw new Error(`cmd.json: no type ${type}`);
}

/** `PlacePart` as a function's name spells it. */
function snake(name: string): string {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
}

function cmdCommands(spec: CmdSpec): CmdEntry[] {
  const table = spec.enums.find((e) => e.name === "Command");
  if (!table) throw new Error("cmd.json has no Command");
  return table.entries.filter((e): e is CmdEntry => !isSection(e));
}

function literal(value: number, format: CmdEnum["format"]): string {
  if (format === "dec") return String(value);
  return `0x${value.toString(16).padStart(format === "hex2" ? 2 : 4, "0")}`;
}

function tsDoc(doc: string[], indent: string): string[] {
  if (doc.length === 1) return [`${indent}/** ${doc[0]} */`];
  return doc.map((l, i) => {
    const body = i === 0 ? `${indent}/** ${l}` : l === "" ? `${indent} *` : `${indent} * ${l}`;
    return i === doc.length - 1 ? `${body} */` : body;
  });
}

function rsDoc(doc: string[], indent: string, mark = "///"): string[] {
  return doc.map((l) => (l === "" ? `${indent}${mark}` : `${indent}${mark} ${l}`));
}

/**
 * What a generated Rust file says above the `allow` that suspends the casing
 * lints. A reader who meets one of these files otherwise has to guess whether
 * the attribute is a reason or an oversight.
 */
const RS_CASING_NOTE = [
  "// Casing here is the spec's and not Rust's: a name is spelled as the JSON",
  "// spells it, so that one record is indexed by one name on both sides. To",
  "// case them the way Rust would is to rename the protocol.",
];

function isSection(e: CmdEntry | CmdSection): e is CmdSection {
  return "section" in e;
}

function cmdTsType(spec: CmdSpec, f: CmdField): string {
  const kind = cmdKind(spec, f.type);
  const one = kind === "number" ? f.type : kind === "bool" ? "boolean" : kind === "str" ? "string" : kind === "extern" && !spec.extern.find((e) => e.name === f.type)!.ts ? "u32" : f.type;
  return f.list ? `${one}[]` : f.optional ? `${one} | null` : one;
}

/** A field as the shell writes it into a packet. */
function cmdTsWrite(spec: CmdSpec, f: CmdField, value: string): string {
  const kind = cmdKind(spec, f.type);
  if (f.list || kind === "record") throw new Error(`cmd.json: the shell writes no ${f.list ? "list" : "record"} (${f.name})`);
  const method = kind === "number" ? f.type : kind === "bool" ? "bool" : kind === "str" ? "str" : "u32";
  return `.${method}(${value})`;
}

/** A field as the shell reads it out of the branch `b`, at `i`. */
function cmdTsRead(spec: CmdSpec, f: CmdField, i: number): string {
  const kind = cmdKind(spec, f.type);
  if (f.list) {
    if (kind === "record") return `list(b.at_branch(${i}), read_${snake(f.type)})`;
    if (kind !== "number") throw new Error(`cmd.json: a list of ${f.type} (${f.name}) is not written`);
    const array = { u32: "U32Array", u64: "U64Array", f64: "F64Array" }[f.type as "u32" | "u64" | "f64"];
    if (!array) throw new Error(`cmd.json: a list of ${f.type} (${f.name}) is not packed`);
    return `Array.from(b.at(${i}).get<${array}>())`;
  }
  if (kind === "record") return f.optional ? `optional(b.at(${i}), read_${snake(f.type)})` : `read_${snake(f.type)}(b.at_branch(${i}))`;
  return `b.at(${i}).get<${cmdTsType(spec, f)}>()`;
}

function cmdTs(spec: CmdSpec): string {
  const out = ["// @pxl/web/kernel/abi/cmd.spec.ts", "//", "// GENERATED by script/spec.ts from cmd.json — do not edit.", "", "/**", ...spec.doc.map((l) => (l === "" ? " *" : ` * ${l}`)), " */"];
  // What the records and the commands are written as, after the enums they
  // name; the imports are what that uses, so an unused one is never written.
  const tail: string[] = [];
  const words = new Set<string>();
  const arrays = new Set<string>(["U8Array"]);
  const fieldLines = (fields: CmdField[]) =>
    fields.flatMap((f) => {
      if (CMD_NUMBERS.has(f.type)) words.add(f.type);
      if (f.list && CMD_NUMBERS.has(f.type)) arrays.add(`${f.type.toUpperCase()}Array`);
      if (cmdKind(spec, f.type) === "extern" && !spec.extern.find((e) => e.name === f.type)!.ts) words.add("u32");
      return [...(f.doc ? tsDoc(f.doc, "  ") : []), `  ${f.name}: ${cmdTsType(spec, f)};`];
    });
  const reader = (name: string, type: string, fields: CmdField[]) => [
    `export function read_${name}(b: BranchCursor): ${type} {`,
    `  return { ${fields.map((f, i) => `${f.name}: ${cmdTsRead(spec, f, i)}`).join(", ")} };`,
    "}",
  ];
  for (const r of spec.records) {
    tail.push("", ...(r.doc ? tsDoc(r.doc, "") : []), `export interface ${r.name} {`, ...fieldLines(r.fields), "}", "", ...reader(snake(r.name), r.name, r.fields));
  }
  for (const c of cmdCommands(spec)) {
    const name = pascal(c.name);
    if (c.packet) {
      tail.push("", `/** Command.${c.name}'s packet. */`, `export interface ${name} {`, ...fieldLines(c.packet), "}", "");
      tail.push(`export function encode_${c.name.toLowerCase()}(p: ${name}): U8Array {`, `  return packet((b) => b${c.packet.map((f) => cmdTsWrite(spec, f, `p.${f.name}`)).join("")});`, "}");
    }
    if (c.signal) {
      tail.push("", `/** Command.${c.name}'s signal. */`, `export interface ${name}Signal {`, ...fieldLines(c.signal), "}", "", ...reader(c.name.toLowerCase(), `${name}Signal`, c.signal));
    }
  }
  out.push(
    "",
    'import type { BranchCursor, NodeCursor } from "@xpute/core/wire/xtp/mod.ts";',
    `import type { ${[...arrays].sort().join(", ")} } from "@xpute/core/abi/array.ts";`,
    `import type { ${[...words].sort().join(", ")} } from "@xpute/core/abi/word.ts";`,
    'import { packet } from "@xpute/runtime/ipc/doorbell.ts";',
  );
  for (const x of spec.extern) if (x.ts) out.push(`import type { ${x.name} } from "${x.ts}";`);
  tail.unshift(
    "",
    "/** A branch of records, each read by `read`. */",
    "function list<T>(b: BranchCursor, read: (b: BranchCursor) => T): T[] {",
    "  return Array.from({ length: b.len }, (_, k) => read(b.at_branch(k)));",
    "}",
    "",
    "/** A record that may be absent. */",
    "function optional<T>(c: NodeCursor, read: (b: BranchCursor) => T): T | null {",
    "  const present = c.opt();",
    "  return present === undefined ? null : read(present.as_branch());",
    "}",
  );
  for (const e of spec.enums) {
    out.push("");
    if (e.doc) out.push(...tsDoc(e.doc, ""));
    out.push(`export const enum ${e.name} {`);
    let first = true;
    for (const entry of e.entries) {
      if (isSection(entry)) {
        out.push("", `  // ---- ${entry.section} ----`);
        if (entry.doc) out.push("  //", ...entry.doc.map((l) => (l === "" ? "  //" : `  // ${l}`)));
        out.push("");
      } else {
        if (entry.doc) out.push(...tsDoc(entry.doc, "  ").map((l, i) => (i === 0 ? l : l.replace(/^ {2}\*/, "   *"))));
        out.push(`  ${entry.name} = ${literal(entry.value, e.format)},`);
      }
      first = false;
    }
    void first;
    out.push("}");
  }
  out.push(...tail);
  return out.join("\n") + "\n";
}

function pascal(name: string): string {
  return name.toLowerCase().split("_").map((w) => w[0].toUpperCase() + w.slice(1)).join("");
}

const CRUD: Record<number, string> = { 1: "CREATE", 2: "READ", 3: "UPDATE", 4: "DELETE" };

/** Whether a record, as Rust writes it, borrows what it carries. */
function cmdRsBorrows(spec: CmdSpec, fields: CmdField[]): boolean {
  return fields.some((f) => {
    const kind = cmdKind(spec, f.type);
    return f.list || kind === "str" || (kind === "record" && cmdRsBorrows(spec, spec.records.find((r) => r.name === f.type)!.fields));
  });
}

/** A record's or a signal's type as a field names it: with its lifetime, or
 * with a fresh one where it is an item a list hands out. */
function cmdRsRecord(spec: CmdSpec, name: string, lifetime: string): string {
  return cmdRsBorrows(spec, spec.records.find((r) => r.name === name)!.fields) ? `${name}<${lifetime}>` : name;
}

/** The path a field's enum is named by. */
function cmdRsEnum(spec: CmdSpec, type: string): string {
  return spec.extern.find((e) => e.name === type)?.rs ?? type;
}

/** A field as the kernel reads it, from a packet the shell wrote. */
function cmdRsReadType(spec: CmdSpec, f: CmdField): string {
  const kind = cmdKind(spec, f.type);
  if (f.list || kind === "record") throw new Error(`cmd.json: the kernel reads no ${f.list ? "list" : "record"} (${f.name})`);
  const one = kind === "number" ? f.type : kind === "str" ? "&'a str" : kind === "bool" ? "bool" : cmdRsEnum(spec, f.type);
  return f.optional ? `Option<${one}>` : one;
}

/** The expression that reads one field out of the cursor `c`. */
function cmdRsReadOne(spec: CmdSpec, f: CmdField, c: string): string {
  const kind = cmdKind(spec, f.type);
  if (kind === "number") return f.type === "u64" ? `${c}.get_u64()?` : f.type === "f64" ? `${c}.get_number()?` : `${c}.get_number()? as ${f.type}`;
  if (kind === "bool") return `${c}.get_bool()?`;
  if (kind === "str") return `${c}.get_str_ref()?`;
  const decl = spec.enums.find((e) => e.name === f.type) ?? spec.extern.find((e) => e.name === f.type)!;
  const path = cmdRsEnum(spec, f.type);
  const number = `${c}.get_number()? as u32`;
  const member = "repr" in decl && decl.repr === "u8" ? `u8::try_from(${number}).ok().and_then(${path}::of)` : `${path}::of(${number})`;
  return decl.unknown ? `${member}.unwrap_or(${path}::${decl.unknown})` : `${member}.ok_or_else(|| unknown("${f.type}"))?`;
}

function cmdRsRead(spec: CmdSpec, f: CmdField, i: number): string {
  return f.optional ? `match f.at(${i})?.opt() { Some(c) => Some(${cmdRsReadOne(spec, f, "c")}), None => None }` : cmdRsReadOne(spec, f, `f.at(${i})?`);
}

/** A field as the kernel writes it. A string is anything that displays, since
 * what it names often lies in no string, and a list is a function handing out
 * its items, which the writer walks twice: once to count them, since a branch's
 * table precedes its children, and once to write them. */
function cmdRsWriteType(spec: CmdSpec, f: CmdField): string {
  const kind = cmdKind(spec, f.type);
  if (f.list) {
    if (kind === "number") return `&'a [${f.type}]`;
    if (kind !== "record") throw new Error(`cmd.json: a list of ${f.type} (${f.name}) is not written`);
    return `&'a dyn Fn(&mut dyn FnMut(&${cmdRsRecord(spec, f.type, "'_")}))`;
  }
  const one = kind === "number" ? f.type : kind === "bool" ? "bool" : kind === "str" ? "&'a dyn Display" : kind === "record" ? cmdRsRecord(spec, f.type, "'a") : cmdRsEnum(spec, f.type);
  return f.optional ? `Option<${one}>` : one;
}

function cmdRsWrite(spec: CmdSpec, f: CmdField): string {
  const kind = cmdKind(spec, f.type);
  const v = `self.${f.name}`;
  if (f.list) {
    if (kind === "number") return `w.${f.type}_array(Some(${v}));`;
    const fields = `${f.type}::FIELDS`;
    return [
      "let mut n = 0;",
      `(${v})(&mut |_| n += 1);`,
      `w.branch(n, |w| (${v})(&mut |item| {`,
      `    w.branch(${fields}, |w| item.write(w));`,
      "}));",
    ].join("\n");
  }
  if (kind === "record") {
    const fields = `${f.type}::FIELDS`;
    return f.optional ? `match &${v} { Some(r) => w.branch(${fields}, |w| r.write(w)), None => w.no_branch() };` : `w.branch(${fields}, |w| ${v}.write(w));`;
  }
  if (kind === "str") return f.optional ? `match ${v} { Some(s) => w.str_display(s), None => w.str(None) };` : `w.str_display(${v});`;
  if (kind === "enum" || kind === "extern") return f.optional ? `w.u32(${v}.map(|m| m as u32));` : `w.u32(Some(${v} as u32));`;
  const method = kind === "bool" ? "bool" : f.type;
  return f.optional ? `w.${method}(${v});` : `w.${method}(Some(${v}));`;
}

function cmdRs(spec: CmdSpec): string {
  const out = ["// pxl-kernel/abi/cmd.spec.rs", "//", "// GENERATED by script/spec.ts from cmd.json — do not edit.", ""];
  out.push(...rsDoc(spec.doc, "", "//!"));
  out.push(
    "",
    "// A list a record carries is a function handing out its items, whose",
    "// type is long to spell and is spelled once, here, for each.",
    "#![allow(non_camel_case_types, clippy::type_complexity)]",
    "",
    "use core::fmt::Display;",
    "",
    "use xpute_core::status::errno::Errno;",
    "use xpute_core::status::error::MarshalError;",
    "use xpute_core::wire::xtp::{PacketWriter, TreeReader};",
    "",
    "/// A number an enum does not give out, where the enum has no member it reads as.",
    "fn unknown(of: &str) -> MarshalError {",
    "    MarshalError::new(Errno::EINVAL, Some(of), None)",
    "}",
  );
  for (const e of spec.enums) {
    const members = e.entries.filter((m): m is CmdEntry => !isSection(m));
    out.push("");
    if (e.doc) out.push(...rsDoc(e.doc, ""));
    out.push(`#[derive(Clone, Copy, Debug, ${e.default ? "Default, " : ""}PartialEq, Eq)]`, "#[repr(u32)]", `pub enum ${e.name} {`);
    for (const entry of e.entries) {
      if (isSection(entry)) {
        out.push("", `    // ---- ${entry.section} ----`);
        if (entry.doc) out.push("    //", ...entry.doc.map((l) => (l === "" ? "    //" : `    // ${l}`)));
      } else {
        if (entry.doc) out.push(...rsDoc(entry.doc, "    "));
        if (entry.name === e.default) out.push("    #[default]");
        out.push(`    ${entry.name} = ${literal(entry.value, e.format)},`);
      }
    }
    out.push("}", "");
    out.push(
      `impl ${e.name} {`,
      `    pub const ALL: [${e.name}; ${members.length}] = [${members.map((m) => `${e.name}::${m.name}`).join(", ")}];`,
      "",
      "    /// The member a number names, or none for a number nobody gave out.",
      `    pub fn of(number: u32) -> Option<${e.name}> {`,
      `        ${e.name}::ALL.into_iter().find(|m| *m as u32 == number)`,
      "    }",
      "}",
    );
  }
  const writer = (name: string, doc: string[] | undefined, fields: CmdField[]) => {
    const borrows = cmdRsBorrows(spec, fields);
    out.push("", ...(doc ? rsDoc(doc, "") : []), `pub struct ${name}${borrows ? "<'a>" : ""} {`);
    for (const f of fields) out.push(...(f.doc ? rsDoc(f.doc, "    ") : []), `    pub ${f.name}: ${cmdRsWriteType(spec, f)},`);
    out.push("}", "", `impl ${name}${borrows ? "<'_>" : ""} {`, "    /// The children it is written as.", `    pub const FIELDS: u32 = ${fields.length};`, "");
    out.push("    pub fn write(&self, w: &mut PacketWriter) {", ...fields.flatMap((f) => cmdRsWrite(spec, f).split("\n").map((l) => `        ${l}`)), "    }", "}");
  };
  for (const r of spec.records) writer(r.name, r.doc, r.fields);
  for (const c of cmdCommands(spec)) {
    const name = pascal(c.name);
    if (c.packet) {
      const borrows = c.packet.some((f) => cmdKind(spec, f.type) === "str");
      const lt = borrows ? "<'a>" : "";
      out.push("", `/// Command::${c.name}'s packet.`, "#[derive(Clone, Copy, Debug)]", `pub struct ${name}${lt} {`);
      for (const f of c.packet) out.push(...(f.doc ? rsDoc(f.doc, "    ") : []), `    pub ${f.name}: ${cmdRsReadType(spec, f)},`);
      out.push("}", "", `impl${lt} ${name}${lt} {`);
      out.push(
        `    pub fn read(pkt: &${borrows ? "'a " : ""}[u8]) -> Result<${name}${lt}, MarshalError> {`,
        "        let f = TreeReader::new(pkt)?.read_branch()?;",
        `        Ok(${name} {`,
        ...c.packet.map((f, i) => `            ${f.name}: ${cmdRsRead(spec, f, i)},`),
        "        })",
        "    }",
        "}",
      );
    }
    if (c.signal) writer(`${name}Signal`, [`Command::${c.name}'s signal.`], c.signal);
  }
  out.push("", "#[cfg(test)]", '#[path = "cmd.test.rs"]', "mod test;");
  return out.join("\n") + "\n";
}

/// The command table's own test, beside it rather than inside it: every `.rs`
/// in the crates keeps its tests in a `.test.rs` at the same path, and a
/// generated file is no exception.
function cmdTestRs(spec: CmdSpec): string {
  const cmds: CmdEntry[] = [];
  const minors = new Map<string, CmdEnum>();
  const majors = new Map<number, string>();
  for (const e of spec.enums) {
    for (const entry of e.entries) {
      if (isSection(entry)) continue;
      if (e.name === "Command") cmds.push(entry);
      else if (e.name === "Major") majors.set(entry.value, entry.name);
      else if (e.name.endsWith("Minor")) minors.set(e.name, e);
    }
  }
  const out: string[] = [
    "// pxl-kernel/abi/cmd.test.rs",
    "//",
    "// GENERATED by script/spec.ts from cmd.json — do not edit.",
    "",
    "use super::*;",
    "use xpute_core::abi::cmd::{cmd, Minor, MINOR_OWN};",
    "",
    "#[test]",
    "fn every_number_is_its_major_and_minor() {",
    "    use Command::*;",
    "    let parts = [",
  ];
  for (const c of cmds) {
    if (c.value === 0) continue;
    const major = majors.get(c.value >> 8);
    if (!major) throw new Error(`cmd.json: ${c.name} has no major`);
    const minorValue = c.value & 0xff;
    let minor: string;
    if (minorValue < 0x10) {
      if (!CRUD[minorValue]) throw new Error(`cmd.json: ${c.name}'s minor is no CRUD verb`);
      minor = `Minor::${CRUD[minorValue]}`;
    } else {
      const table = minors.get(`${pascal(major)}Minor`);
      const entry = table?.entries.find((m): m is CmdEntry => !isSection(m) && m.value === minorValue);
      if (!table || !entry) throw new Error(`cmd.json: ${c.name}'s minor is in no ${pascal(major)}Minor`);
      minor = `${table.name}::${entry.name}`;
    }
    out.push(`        (${c.name}, Major::${major} as u32, ${minor} as u32),`);
  }
  out.push(
    "    ];",
    "    for (c, major, minor) in parts {",
    '        assert_eq!(c as u32, cmd(major, minor), "{c:?}");',
    "        assert_eq!(Command::of(c as u32), Some(c));",
    "    }",
    "    assert_eq!(Command::of(0x0fff), None);",
    "}",
    "",
    "/// Nothing computes a minor at run time — every number here is a constant",
    "/// by the time it is on the wire — so what this catches is a subject's own",
    "/// verb written below MINOR_OWN in the JSON, where it would answer to the",
    "/// same number as one of the four every subject has.",
    "#[test]",
    "fn a_subject_s_own_verbs_start_past_the_four_every_subject_has() {",
    "    let own: [(&str, u32); OWN_MINORS] = [",
  );
  let ownCount = 0;
  for (const table of minors.values()) {
    for (const entry of table.entries) {
      if (isSection(entry)) continue;
      ownCount += 1;
      out.push(`        ("${table.name}::${entry.name}", ${table.name}::${entry.name} as u32),`);
    }
  }
  out.push(
    "    ];",
    "    for (name, value) in own {",
    '        assert!(value >= MINOR_OWN, "{name} = {value:#x} is below MINOR_OWN");',
    "    }",
    "}",
  );
  return out.join("\n").replace("OWN_MINORS", String(ownCount)) + "\n";
}

async function cmdOutputs(): Promise<Output[]> {
  const spec: CmdSpec = JSON.parse(await Deno.readTextFile(join(root, "spec/pxl/web/kernel/abi/cmd.json")));
  return [
    { path: "apps/web/src/kernel/abi/cmd.spec.ts", text: cmdTs(spec) },
    { path: "crates/pxl/web/kernel/src/abi/cmd.spec.rs", text: cmdRs(spec) },
    { path: "crates/pxl/web/kernel/src/abi/cmd.test.rs", text: cmdTestRs(spec) },
  ];
}

// ---- the cell catalogs ----

type ColorSpec = { _meta?: { description?: string; source?: string } } & Record<string, Record<string, { hex: string; rgb?: number; note?: string }>>;

async function readJson<T>(path: string): Promise<T> {
  return JSON.parse(await Deno.readTextFile(join(root, path)));
}

/** `#RRGGBB` as the one number it is. The JSON is written in hex because a
 * person picks a colour that way; nothing downstream reads it back as text. */
function packed(hex: string): number {
  const m = /^#([0-9a-fA-F]{6})$/.exec(hex);
  if (m === null) throw new Error(`color JSON: "${hex}" is not #RRGGBB`);
  return parseInt(m[1], 16);
}

/** A `$type: "rgb"` leaf: written `#RRGGBB` because a person picks a colour
 * that way, emitted as the one number it is because nothing that lights or
 * blends with it wants text. */
function rgbLiteral(hex: string): string {
  return `0x${packed(hex).toString(16).padStart(6, "0")}`;
}

async function colorOutputs(): Promise<Output[]> {
  const spec = await readJson<ColorSpec>("spec/pxl/spec/cell/color.json");
  const head = [
    "//",
    "// GENERATED by script/spec.ts from color.json — do not edit.",
    "//",
    "// The colour of every cube type: what a cube *is* to the eye when no",
    "// texture is drawn over it — the minimap and everything drawn with the",
    "// pack off read this, the coarse tiers included, which summarise it the",
    "// way they summarise the tiles with the pack on",
    "// (pxl-render mesh/chunk_mesher.rs). The shipped palette carries it as",
    "// each row's `color`. A texture pack decorates these; it does not",
    "// replace them, which is why this table lives with the type numbering in",
    "// spec/cell rather than beside the pack's images.",
  ];
  const packedSpec: ColorSpec = { ...spec };
  for (const [ch, entries] of Object.entries(spec)) {
    if (ch === "_meta") continue;
    packedSpec[ch] = Object.fromEntries(Object.entries(entries as Record<string, { hex: string; note?: string }>).map(([n, e]) => [n, { ...e, rgb: packed(e.hex) }]));
  }
  const ts = [
    "// @pxl/web/catalog/cell/color.spec.ts",
    ...head,
    "",
    "export default " + JSON.stringify(packedSpec, null, 2).replace(/"([A-Za-z_][A-Za-z0-9_]*)":/g, "$1:").replace(/\n(\s*)\}/g, ",\n$1}").replace(/\n(\s*)\]/g, ",\n$1]") + ";",
  ];
  // What the Rust holds is each channel's colour as a function of its type's
  // number, matched on the catalog's own constants, so nothing there finds a
  // channel or a type by its name. A ground channel falls back to `common`'s,
  // which is what a sentinel a baked cell can hold (VOID, UNKNOWN) shows as.
  const types = await readJson<CubeSpec>("spec/pxl/spec/cell/cube_type.json");
  const rs = ["// pxl-spec/cell/color.spec.rs", ...head, "", ...RS_CASING_NOTE, "", "use super::cube_type::{cube_types, GroundChannelKey};", "use super::r#type::Swatch;"];
  for (const [ch, entries] of Object.entries(spec)) {
    if (ch === "_meta") continue;
    const channel = types.channels.find((c) => c.name === ch);
    if (!channel) throw new Error(`color.json: ${ch} is no channel cube_type.json has`);
    rs.push("", `/// \`${ch}\`'s colour for a type, by its number.`, `pub const fn ${ch}(t: u8) -> Option<Swatch> {`, "    match t {");
    for (const [name, entry] of Object.entries(entries as Record<string, { hex: string; note?: string }>)) {
      if (!channel.entries.some((e) => "name" in e && e.name === name)) throw new Error(`color.json: ${ch}.${name} is no type cube_type.json has`);
      if (entry.note !== undefined) rs.push(`        // ${entry.note}`);
      rs.push(`        cube_types::${ch}::${rsIdent(name)} => Some(Swatch::new(${JSON.stringify(entry.hex)}, ${rgbLiteral(entry.hex)})),`);
    }
    rs.push("        _ => None,", "    }", "}");
  }
  rs.push(
    "",
    "/// A ground channel's colour for a type: its own, or `common`'s for a",
    "/// sentinel a baked cell can hold.",
    "pub const fn ground(channel: GroundChannelKey, t: u8) -> Option<Swatch> {",
    "    let own = match channel {",
    ...GROUND_CHANNELS.map((name) => `        GroundChannelKey::${name.toUpperCase()} => ${name}(t),`),
    "    };",
    "    match own {",
    "        Some(swatch) => Some(swatch),",
    "        None => common(t),",
    "    }",
    "}",
  );
  return [
    { path: "apps/web/src/catalog/cell/color.spec.ts", text: ts.join("\n") + "\n" },
    { path: "crates/pxl/spec/src/cell/color.spec.rs", text: rs.join("\n") + "\n" },
  ];
}

type CubeEntry = { name: string; value: number; recess?: number; doc?: string[] } | { section: string };
interface CubeSpec {
  base: { channel: string; type: string; doc: string[] };
  channels: { name: string; note?: string[]; entries: CubeEntry[] }[];
}

/// The ground channels, numbered by their place here: that number is
/// `GroundChannelKey`'s, which is a channel's identity in what the object
/// store holds, and the field a packed cell holds it in. Appending one is
/// free; moving one renumbers stored placements.
const GROUND_CHANNELS = ["land", "water", "structure", "transport"];

/// A ground cell's four channels packed into one word, for Rust alone: the
/// client and the bake both compile it, and the shell never decodes a page.
///
/// A channel's field is a dense index into its own table, not the catalog's
/// number, because the catalog numbers an identity rather than a sequence —
/// `land::sea` is 129 — and a placed cube already names one of them where a
/// browser can see it. The table is what turns one back into the other, and
/// the field's width comes out of the table's length, so a new type widens
/// its own field and shifts the ones above it with nothing edited by hand
/// (issue/20260923-a-ground-cell-is-fourteen-bits.md).
function groundPack(spec: CubeSpec): { rs: string[]; ts: string[] } {
  const tables = GROUND_CHANNELS.map((name) => {
    const ch = spec.channels.find((c) => c.name === name);
    if (!ch) throw new Error(`cube_type.json has no ${name} channel`);
    const entries = ch.entries.filter((e): e is Extract<CubeEntry, { name: string }> => "name" in e);
    // VOID first, so a cell of nothing is a word of zero, and UNKNOWN last,
    // so it is where a byte the channel does not give out is sent.
    return { name, values: ["common::void", ...entries.map((e) => `${name}::${e.name}`), "common::unknown"] };
  });
  const bits = tables.map((t) => Math.max(1, 32 - Math.clz32(t.values.length - 1)));
  const shifts: number[] = [];
  bits.reduce((at, b) => (shifts.push(at), at + b), 0);
  const cellBits = bits.reduce((a, b) => a + b, 0);

  const numberOf = new Map<string, number>();
  for (const ch of spec.channels) {
    for (const e of ch.entries) if ("name" in e) numberOf.set(`${ch.name}::${e.name}`, e.value);
  }
  const inverse = tables.map((t) => {
    const row = new Array(256).fill(t.values.length - 1); // UNKNOWN
    t.values.forEach((v, i) => {
      const n = numberOf.get(v);
      if (n === undefined) throw new Error(`no number for ${v}`);
      row[n] = i;
    });
    return row;
  });

  const rows = (row: number[]) => {
    const out: string[] = [];
    for (let i = 0; i < 256; i += 32) out.push(`            ${row.slice(i, i + 32).join(", ")},`);
    return out;
  };

  const ts = [
    "/** A ground cell's four channels in one word: each a dense index into its",
    " * own table, whose entries are the catalog's own numbers. VOID is index 0",
    " * and UNKNOWN the last. The widths are the tables' lengths and nothing",
    " * else; pxl-spec's bake/pack.rs is the reader that knows a field has a",
    " * place, and this is here for whoever writes a page the kernel will read. */",
    "export const ground_pack = {",
    `  channels: [${tables.map((t) => JSON.stringify(t.name)).join(", ")}],`,
    `  number: [${tables.map((t) => `[${t.values.map((v) => numberOf.get(v)).join(", ")}]`).join(", ")}],`,
    `  index: [${inverse.map((row) => `[${row.join(", ")}]`).join(", ")}],`,
    `  bits: [${bits.join(", ")}],`,
    `  shift: [${shifts.join(", ")}],`,
    `  cell_bits: ${cellBits},`,
    "} as const;",
  ];
  const rs = [
    "/// A ground cell's four channels in one word: each a dense index into its",
    "/// own table, whose entries are the catalog's own numbers. VOID is index 0",
    "/// and UNKNOWN the last, so a cell nothing painted is a word of zero and a",
    "/// byte no channel gives out reads as UNKNOWN rather than as a type.",
    "///",
    "/// The widths are the tables' lengths and nothing else. Everything here is",
    "/// walked through `bake/ground.rs`'s pack and unpack, which are the only",
    "/// readers that know a field has a place.",
    "pub mod ground_pack {",
    "    use super::cube_types::*;",
    "",
    "    /// The channels a packed cell holds, in the order their fields sit in",
    "    /// the word, by name for a tool that prints one. A field is found by",
    "    /// `GroundChannelKey`'s number, which is this order.",
    `    pub const CHANNELS: [&str; ${tables.length}] = [${tables.map((t) => JSON.stringify(t.name)).join(", ")}];`,
    "",
    "    /// The catalog number each dense index names, a channel a row.",
    `    pub const NUMBER: [&[u8]; ${tables.length}] = [`,
    ...tables.map((t) => `        &[${t.values.join(", ")}],`),
    "    ];",
    "",
    "    /// The dense index each catalog number takes, a channel a row: the",
    "    /// reverse of NUMBER, and UNKNOWN's index where the channel gives the",
    "    /// number out to nobody.",
    `    pub const INDEX: [[u8; 256]; ${tables.length}] = [`,
    ...inverse.flatMap((row) => ["        [", ...rows(row), "        ],"]),
    "    ];",
    "",
    "    /// What it takes to name one of a channel's entries, and where that",
    "    /// field starts in the word.",
    `    pub const BITS: [u32; ${tables.length}] = [${bits.join(", ")}];`,
    `    pub const SHIFT: [u32; ${tables.length}] = [${shifts.join(", ")}];`,
    "",
    "    /// The bits a ground cell takes, all four fields together.",
    `    pub const CELL_BITS: u32 = ${cellBits};`,
    "}",
  ];
  return { rs, ts };
}

async function cubeTypeOutputs(): Promise<Output[]> {
  const spec = await readJson<CubeSpec>("spec/pxl/spec/cell/cube_type.json");
  const head = [
    "//",
    "// GENERATED by script/spec.ts from cube_type.json — do not edit.",
    "//",
    "// The cube catalog's numbering, every channel a name per value;",
    "// color.json colours it by the same names, and pxl-spec cell/cube.rs says",
    "// what each channel is.",
  ];
  const keyDoc = [
    "A ground channel: one of the four a baked cell is painted in. The number is",
    "its identity — the number a stored value names it by, such as a",
    "terrain-styled cube's — and the field a packed cell holds it in",
    "(`ground_pack`), both written from one list; it is not its place in the",
    "order a cell is painted in (bake/format.rs `GROUND_CH_KEYS`), which can move",
    "without renumbering anything the object store holds.",
  ];
  const keys = GROUND_CHANNELS.map((name, i) => [name.toUpperCase(), i] as const);
  const ts = [
    "// @pxl/web/catalog/cell/cube_type.spec.ts",
    ...head,
    "",
    ...tsDoc(keyDoc, ""),
    "export const enum GroundChannelKey {",
    ...keys.map(([k, i]) => `  ${k} = ${i},`),
    "}",
    "",
    "export const cube_types = {",
  ];
  const enums: string[] = [];
  const rs = ["// pxl-spec/cell/cube_type.spec.rs", ...head, "", ...RS_CASING_NOTE, "#![allow(non_camel_case_types, non_upper_case_globals)]", "", "use super::r#type::Channels;", ""];
  rs.push(...rsDoc(keyDoc, ""), "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]", "#[repr(u8)]", "pub enum GroundChannelKey {", ...keys.map(([k, i]) => `    ${k} = ${i},`), "}", "");
  rs.push(
    "impl GroundChannelKey {",
    `    pub const ALL: [GroundChannelKey; ${keys.length}] = [${keys.map(([k]) => `GroundChannelKey::${k}`).join(", ")}];`,
    "",
    "    /// The channel a stored identity names, the reverse of `self as u8`.",
    "    pub const fn from_id(id: u32) -> Option<GroundChannelKey> {",
    "        match id {",
    ...keys.map(([k, i]) => `            ${i} => Some(GroundChannelKey::${k}),`),
    "            _ => None,",
    "        }",
    "    }",
    "",
    "    /// The same, from the byte a packet carries it as.",
    "    pub const fn of(id: u8) -> Option<GroundChannelKey> {",
    "        GroundChannelKey::from_id(id as u32)",
    "    }",
    "}",
    "",
  );
  rs.push(
    "/// The catalog: a module per channel, each type a constant, and `ENTRIES`",
    "/// the channel as the object's keys walk it. `CHANNELS` is `cube_types`",
    "/// itself, walked.",
    "pub mod cube_types {",
  );
  spec.channels.forEach((ch, ci) => {
    if (ci > 0) ts.push("", `  // ---- ${ch.name} ----`);
    for (const l of ch.note ?? []) ts.push(l === "" ? "  //" : `  // ${l}`);
    if (ci > 0) ts.push("");
    ts.push(`  ${ch.name}: {`);
    if (ci > 0) rs.push("");
    for (const l of ch.note ?? []) rs.push(l === "" ? "    //" : `    // ${l}`);
    rs.push(`    pub mod ${ch.name} {`);
    const names: string[] = [];
    const recessed: [string, number][] = [];
    const types: { name: string; lit: string; doc?: string[] }[] = [];
    ch.entries.forEach((e, ei) => {
      if ("section" in e) {
        if (ei > 0) {
          ts.push("");
          rs.push("");
        }
        ts.push(`    // ---- ${e.section} ----`, "");
        rs.push(`        // ---- ${e.section} ----`, "");
        return;
      }
      if (e.doc) ts.push(...tsDoc(e.doc, "    ").map((l, i) => (i === 0 ? l : l.replace(/^ {4}\*/, "     *"))));
      if (e.doc) rs.push(...rsDoc(e.doc, "        "));
      const lit = ch.name === "common" ? `0x${e.value.toString(16).padStart(2, "0")}` : String(e.value);
      ts.push(`    ${e.name}: ${lit},`);
      rs.push(`        pub const ${e.name}: u8 = ${lit};`);
      names.push(e.name);
      if (e.recess !== undefined) recessed.push([e.name, e.recess]);
      types.push({ name: e.name, lit, doc: e.doc });
    });
    ts.push("  },");
    rs.push("", `        pub const ENTRIES: &[(&str, u8)] = &[`, ...names.map((n) => `            (${JSON.stringify(n)}, ${n}),`), "        ];");
    // Rust alone: the recess is a height, and heights are the kernel's.
    rs.push(
      "",
      "        /// How far below its sampled height a cell showing the type sits, in",
      "        /// units; a type not listed sits at it.",
      `        pub const RECESS: &[(u8, i16)] = &[${recessed.map(([n, r]) => `(${n}, ${r})`).join(", ")}];`,
      "    }",
    );
    // The same channel as one type, for Rust, which names a member of a closed
    // set with an enum rather than with the text the JSON spells it out under.
    // TypeScript already has this: `cube_types.land` IS the closed set there,
    // its keys checked by `keyof typeof`.
    const e = pascal(ch.name);
    enums.push("");
    for (const l of ch.note ?? []) enums.push(l === "" ? "    //" : `    // ${l}`);
    enums.push("    #[derive(Clone, Copy, Debug, PartialEq, Eq)]", "    #[repr(u8)]", `    pub enum ${e} {`);
    for (const t of types) {
      if (t.doc) enums.push(...rsDoc(t.doc, "        "));
      enums.push(`        ${t.name} = ${t.lit},`);
    }
    enums.push("    }", "");
    enums.push(
      `    impl ${e} {`,
      "        /// Every type the channel gives out, in the JSON's own order.",
      `        pub const ALL: [${e}; ${types.length}] = [`,
      ...types.map((t) => `            ${e}::${t.name},`),
      "        ];",
      "",
    );
    enums.push(
      "        /// The type a number names, or none for a number the channel",
      "        /// does not give out.",
      `        pub fn of(value: u8) -> Option<${e}> {`,
      `            ${e}::ALL.into_iter().find(|t| *t as u8 == value)`,
      "        }",
      "    }",
    );
  });
  ts.push("} as const satisfies Record<string, Record<string, number>>;");
  rs.push("", "    pub const CHANNELS: &super::Channels = &[", ...spec.channels.map((ch) => `        (${JSON.stringify(ch.name)}, ${ch.name}::ENTRIES),`), "    ];");
  rs.push(
    "",
    ...rsDoc(spec.base.doc, "    "),
    "    pub mod base {",
    `        pub const CHANNEL: super::super::GroundChannelKey = super::super::GroundChannelKey::${spec.base.channel.toUpperCase()};`,
    `        pub const TYPE: u8 = super::${spec.base.channel}::${spec.base.type};`,
    "    }",
  );
  rs.push(...enums, "}");
  const pack = groundPack(spec);
  rs.push("", ...pack.rs);
  ts.push("", ...pack.ts);
  return [
    { path: "apps/web/src/catalog/cell/cube_type.spec.ts", text: ts.join("\n") + "\n" },
    { path: "crates/pxl/spec/src/cell/cube_type.spec.rs", text: rs.join("\n") + "\n" },
  ];
}

// ---- the runtime's fetch failures ----
//
// Errno-shaped: a closed vocabulary both ends read by number, where the
// number is the name and a retired one leaves its gap. Unlike errno each
// entry carries a paragraph, a section groups a range, and the guest wants
// a short `&'static str` per entry for its own report.

interface VocabEntry {
  name: string;
  value: string;
  str: string;
  default?: true;
  doc: string[];
}
interface VocabSection {
  section: string;
}
interface VocabRetired {
  retired: string[];
}
interface VocabSpec {
  doc: string[];
  name: string;
  repr: string;
  entries: (VocabEntry | VocabSection | VocabRetired)[];
  as_str: string[];
}

async function fetchOutputs(): Promise<Output[]> {
  const spec = await readJson<VocabSpec>("spec/xpute/abi/fetch.json");
  const head = ["//", "// GENERATED by script/spec.ts from fetch.json — do not edit.", ""];
  const entries = spec.entries.filter((e): e is VocabEntry => "name" in e);

  const ts = ["// @xpute/runtime/abi/fetch.spec.ts", ...head, ...tsDoc(spec.doc, ""), `export const enum ${spec.name} {`];
  const rs = [
    "// xpute-runtime/abi/fetch.spec.rs",
    ...head,
    ...rsDoc(spec.doc, ""),
    "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]",
    `#[repr(${spec.repr})]`,
    `pub enum ${spec.name} {`,
  ];
  for (const e of spec.entries) {
    if ("section" in e) {
      ts.push("", `  // ---- ${e.section} ----`);
      rs.push("", `    // ---- ${e.section} ----`);
      continue;
    }
    if ("retired" in e) {
      rs.push(...e.retired.map((l) => `    // ${l}`));
      continue;
    }
    ts.push(...tsDoc(e.doc, "  "), `  ${e.name} = ${e.value},`);
    rs.push(...rsDoc(e.doc, "    "));
    if (e.default) rs.push("    #[default]");
    rs.push(`    ${e.name} = ${e.value},`);
  }
  ts.push("}", "");
  rs.push("}", "", `impl ${spec.name} {`, `    pub const ALL: [${spec.name}; ${entries.length}] = [${entries.map((e) => `${spec.name}::${e.name}`).join(", ")}];`, "");
  rs.push(
    "    /// The reason a number names, or none for a number nobody gave out.",
    `    pub fn of(number: ${spec.repr}) -> Option<${spec.name}> {`,
    `        ${spec.name}::ALL.into_iter().find(|f| *f as ${spec.repr} == number)`,
    "    }",
    "",
  );
  rs.push(...rsDoc(spec.as_str, "    "), "    pub fn as_str(self) -> &'static str {", "        match self {");
  for (const e of entries) rs.push(`            ${spec.name}::${e.name} => "${e.str}",`);
  rs.push("        }", "    }", "}", "");

  return [
    { path: "packages/xpute/runtime/src/abi/fetch.spec.ts", text: ts.join("\n") },
    { path: "crates/xpute/runtime/src/abi/fetch.spec.rs", text: rs.join("\n") },
  ];
}

// ---- the kit's errno ----
//
// The numbers cross the ring, so both sides have to agree on all fifty-nine;
// they were two hand-kept tables and nothing checked them against each other.
// The name stays out of this: it is the identifier, and `errname()`-shaped
// output derives it from the symbol rather than from a table (status/error.rs's
// own `stringify!`), so there is nothing here for a reader to drift from.

interface ErrnoEntry {
  name: string;
  value: number;
  doc: string;
}
interface ErrnoSection {
  section: string;
}
interface ErrnoSpec {
  doc: string[];
  entries: (ErrnoEntry | ErrnoSection)[];
}

function isErrnoSection(e: ErrnoEntry | ErrnoSection): e is ErrnoSection {
  return "section" in e;
}

async function errnoOutputs(): Promise<Output[]> {
  const spec = await readJson<ErrnoSpec>("spec/xpute/status/errno.json");
  const head = ["//", "// GENERATED by script/spec.ts from errno.json — do not edit.", "//", ...spec.doc.map((l) => `// ${l}`)];

  const ts = ["// @xpute/core/status/errno.spec.ts", ...head, "", "export const enum Errno {"];
  const rs = ["// xpute-core/status/errno.spec.rs", ...head, "", "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]", "#[repr(i32)]", "pub enum Errno {"];
  // Rust lines up its trailing comments per section, as rustfmt leaves them;
  // TypeScript does not, as deno fmt leaves them.
  let run: ErrnoEntry[] = [];
  const flush = () => {
    const width = Math.max(0, ...run.map((e) => `${e.name} = ${e.value},`.length));
    for (const e of run) rs.push(`    ${`${e.name} = ${e.value},`.padEnd(width)} // ${e.doc}`);
    run = [];
  };
  spec.entries.forEach((e, i) => {
    if (isErrnoSection(e)) {
      flush();
      rs.push("", `    // ---- ${e.section} ----`);
      ts.push("", `  // ---- ${e.section} ----`, "");
      return;
    }
    run.push(e);
    ts.push(`  ${e.name} = ${e.value}, // ${e.doc}`);
    void i;
  });
  flush();
  rs.push("}", "");
  ts.push("}", "");

  const tailDoc = [
    "Conventions:",
    "- errno == Errno::OK  -> success",
    "- errno != Errno::OK  -> failure",
    "- data == None        -> no payload",
  ];
  rs.push(
    "// Result pair: (errno, data)",
    "//",
    ...tailDoc.map((l) => `// ${l}`),
    "//",
    '// `None` is the "no value / no payload" sentinel: the empty result slot,',
    "// nothing about pointers.",
    "pub type ErrnoResult<T> = (Errno, Option<T>);",
  );
  ts.push(
    "// Result pair: [errno, data]",
    "//",
    "// Conventions:",
    "// - errno == Errno.OK  -> success",
    "// - errno != Errno.OK  -> failure",
    "// - data === null      -> no payload",
    "//",
    '// `null` is used as the canonical "no value / no payload" sentinel.',
    "// It does not imply pointer semantics; it is just the empty result slot.",
    "export type ErrnoResult<T> = [Errno, T | null];",
  );
  return [
    { path: "packages/xpute/core/src/status/errno.spec.ts", text: ts.join("\n") + "\n" },
    { path: "crates/xpute/core/src/status/errno.spec.rs", text: rs.join("\n") + "\n" },
  ];
}

interface DecorSpec {
  doc: string[];
  decor_types: Record<string, number>;
}

async function decorOutputs(): Promise<Output[]> {
  const spec = await readJson<DecorSpec>("spec/pxl/spec/cell/decor.json");
  const head = [
    "//",
    "// GENERATED by script/spec.ts from decor.json — do not edit.",
    "//",
    "// [Layer: objects] User-placed decor kind catalog — the cube catalog's own sibling, non-cube content (GLB instances, not a meshed box).",
    "//",
  ];
  const entries = Object.entries(spec.decor_types);
  const ts = [
    "// @pxl/web/catalog/cell/decor.spec.ts",
    ...head,
    ...spec.doc.map((l) => `// ${l}`),
    "export const decor_types = {",
    ...entries.map(([k, v]) => `  ${k}: ${v},`),
    "} as const satisfies Record<string, number>;",
    "",
    "export type DecorId = keyof typeof decor_types;",
  ];
  const rs = [
    "// pxl-spec/cell/decor.spec.rs",
    ...head,
    ...spec.doc.map((l) => `// ${l}`),
    "//",
    ...RS_CASING_NOTE,
    "#[allow(non_upper_case_globals, non_camel_case_types)]",
    "pub mod decor_types {",
    ...entries.map(([k, v]) => `    pub const ${k}: u8 = ${v};`),
    "",
  ];
  // The catalog as one type, as cube_type.json's channels are: the kernel
  // holds a decor kind as this, and its name is the shell's.
  rs.push("    #[derive(Clone, Copy, Debug, PartialEq, Eq)]", "    #[repr(u8)]", "    pub enum Decor {", ...entries.map(([k, v]) => `        ${k} = ${v},`), "    }", "");
  rs.push(
    "    impl Decor {",
    "        /// Every kind the catalog gives out, in the JSON's own order.",
    `        pub const ALL: [Decor; ${entries.length}] = [${entries.map(([k]) => `Decor::${k}`).join(", ")}];`,
    "",
  );
  rs.push(
    "        /// The kind a number names, or none for a number the catalog does not",
    "        /// give out.",
    "        pub fn of(value: u8) -> Option<Decor> {",
    "            Decor::ALL.into_iter().find(|t| *t as u8 == value)",
    "        }",
    "    }",
    "}",
    "",
    "pub type DecorId = decor_types::Decor;",
  );
  return [
    { path: "apps/web/src/catalog/cell/decor.spec.ts", text: ts.join("\n") + "\n" },
    { path: "crates/pxl/spec/src/cell/decor.spec.rs", text: rs.join("\n") + "\n" },
  ];
}

interface PublishedSpec {
  doc: string[];
  PUBLISHED_VERSION: Record<string, string>;
}

async function publishedOutputs(): Promise<Output[]> {
  const spec = await readJson<PublishedSpec>("spec/pxl/spec/bake/published.json");
  const kinds = Object.entries(spec.PUBLISHED_VERSION);
  const head = ["//", "// GENERATED by script/spec.ts from published.json — do not edit.", ""];
  const ts = [
    "// @pxl/web/config/published.spec.ts",
    ...head,
    "/**",
    ...spec.doc.map((l) => (l === "" ? " *" : ` * ${l}`)),
    " */",
    "export const PUBLISHED_VERSION = {",
    ...kinds.map(([k, v]) => `  ${k}: ${JSON.stringify(v)},`),
    "} as const;",
    "",
    "export type BakeKind = keyof typeof PUBLISHED_VERSION;",
    "",
    "/** The kinds by the number the kernel names one by (FETCH_CREATE's `bake`). */",
    `export const BAKE_KINDS: readonly BakeKind[] = [${kinds.map(([k]) => JSON.stringify(k)).join(", ")}];`,
  ];
  const rs = ["// pxl-spec/bake/published.spec.rs", ...head, ...rsDoc(spec.doc, ""), "pub struct PublishedVersion {", ...kinds.map(([k]) => `    pub ${k}: &'static str,`), "}", ""];
  rs.push("pub const PUBLISHED_VERSION: PublishedVersion = PublishedVersion {", ...kinds.map(([k, v]) => `    ${k}: ${JSON.stringify(v)},`), "};", "");
  rs.push(
    "/// The kinds a version is published for: the fields of `PUBLISHED_VERSION`,",
    "/// numbered in their order.",
    "#[allow(non_camel_case_types)]",
    "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
    "#[repr(u32)]",
    "pub enum BakeKind {",
    ...kinds.map(([k]) => `    ${k},`),
    "}",
    "",
    "impl BakeKind {",
    `    pub const ALL: [BakeKind; ${kinds.length}] = [${kinds.map(([k]) => `BakeKind::${k}`).join(", ")}];`,
    "",
    "    /// The kind a number names, or none for a number nobody gave out.",
    "    pub fn of(number: u32) -> Option<BakeKind> {",
    "        BakeKind::ALL.into_iter().find(|k| *k as u32 == number)",
    "    }",
    "}",
    "",
  );
  rs.push(
    "impl PublishedVersion {",
    "    /// `PUBLISHED_VERSION[kind]`.",
    "    pub fn of(&self, kind: BakeKind) -> &'static str {",
    "        match kind {",
    ...kinds.map(([k]) => `            BakeKind::${k} => self.${k},`),
    "        }",
    "    }",
    "}",
  );
  return [
    { path: "apps/web/src/config/published.spec.ts", text: ts.join("\n") + "\n" },
    { path: "crates/pxl/spec/src/bake/published.spec.rs", text: rs.join("\n") + "\n" },
  ];
}

// ---- the POI theme ----
//
// The shell reads the table by a place's category key, which a search result
// carries as text (the data spec above writes that). What the Rust reads is a
// kind as the number a POI page carries it by, so it is written as one list in
// the JSON's order: the bake numbers a place's category by it, and the kernel
// draws a badge from it.

interface PoiThemeSpec {
  $doc: string[];
  CATEGORY_THEME: Record<string, { icon: string; color: string } | string[]>;
  DEFAULT_THEME: { icon: string; color: string };
}

async function poiThemeOutputs(): Promise<Output[]> {
  const json = "spec/pxl/spec/bake/poi_theme.json";
  const spec = await readJson<PoiThemeSpec>(json);
  const kinds = Object.entries(spec.CATEGORY_THEME).filter(([k]) => !k.startsWith("$")) as [string, { icon: string; color: string }][];
  // A kind is a byte on the wire, and 0 is none.
  if (kinds.length > 255) throw new Error(`${json}: ${kinds.length} kinds do not fit a byte beside none`);
  const theme = (t: { icon: string; color: string }) => `PoiTheme { icon: ${JSON.stringify(t.icon)}, color: 0x${packed(t.color).toString(16).padStart(6, "0")}ff }`;
  const rs = [
    "// pxl-spec/bake/poi_theme.spec.rs",
    "//",
    `// GENERATED by script/spec.ts from poi_theme.json — do not edit.`,
    "//",
    ...spec.$doc.map((l) => (l === "" ? "//" : `// ${l}`)),
    "",
    "/// How a kind of place is drawn: its icon, an emoji, and its colour as",
    "/// `0xRRGGBBAA`.",
    "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
    "pub struct PoiTheme {",
    "    pub icon: &'static str,",
    "    pub color: u32,",
    "}",
    "",
    "/// Every kind by its key, kind k at `KINDS[k - 1]`.",
    `pub const KINDS: [(&str, PoiTheme); ${kinds.length}] = [`,
    ...kinds.map(([k, t]) => `    (${JSON.stringify(k)}, ${theme(t)}),`),
    "];",
    "",
    "/// A place of no kind the table names.",
    `pub const DEFAULT_THEME: PoiTheme = ${theme(spec.DEFAULT_THEME)};`,
  ];
  return [{ path: "crates/pxl/spec/src/bake/poi_theme.spec.rs", text: rs.join("\n") + "\n" }];
}

// ---- data specs ----
//
// A data spec is a JSON file of named values, written as a Rust module of
// nested `pub mod`s of constants, and — where the shell still reads the same
// values — as a TypeScript module of `as const` objects. Most of these are the
// kernel's alone, so they name no `ts` at all.
//
// A leaf is a number, a string, a boolean or an array of one kind of them; a
// number is an f64 unless a `$type` above it says otherwise ("u8".."u64",
// "i32", "f32"), and a u64 is written as a hex string.
//
// A closed set of names — a list of them, an object of entries, or names an
// integer `$type` numbers — is also an enum, as cube_type.json's channels
// are. A string whose `$type` names an enum (a path, or a name this spec
// declares) is that enum's member; `"rgb"` is its packed colour and `"hash"`
// its FNV-1a. Any other string is a `&str`. A value that carries
// its reason is `{ "$value": …, "$doc": [lines] }`; an object's own reason is
// its `$doc` key. The file's `$doc` heads the module.

type DataNode = number | string | boolean | DataNode[] | { [key: string]: DataNode };
type DataSpec = { $doc?: string[]; $ts?: string; $rs?: string; $tsHeader?: string; $rsHeader?: string } & Record<string, DataNode>;

/** `enums`: the spec's closed sets are also written as Rust enums (see rsItems). */
/** `tsNames`: the top-level names the TypeScript reads; the rest of the table
 * is the Rust's alone and is not written into the shell. Absent where the
 * TypeScript reads all of it. */
/** `mod` is the Rust module the router declares for the file, where it is not the file's stem. */
/** `enums` may instead list the top-level lists of names that are enums, for
 * a spec whose other tables are structures and not sets. */
const DATA_SPECS: { json: string; ts?: string; rs?: string; mod?: string; enums?: true | string[]; tsNames?: string[] }[] = [
  { json: "spec/pxl/spec/bake/poi_theme.json", ts: "apps/web/src/feature/search/poi_theme.spec.ts" },
  {
    json: "spec/pxl/web/kernel/layer/character/variant.json",
    ts: "apps/web/src/feature/character/variant.spec.ts",
    rs: "crates/pxl/web/kernel/src/layer/character/variant.spec.rs",
    mod: "variant_spec",
    enums: true,
    tsNames: ["groups", "DEFAULT_PERSON_VARIANT_KEY", "DEFAULT_PET_VARIANT_KEY", "PERSON_GENDER_PREFIXES"],
  },
  { json: "spec/pxl/web/render/gpu/api.json", ts: "apps/web/src/kernel/render/gpu/api.spec.ts", rs: "crates/pxl/web/render/src/gpu/api.spec.rs", mod: "api_spec" },
  { json: "spec/pxl/web/kernel/abi/host.json", ts: "apps/web/src/kernel/abi/host.spec.ts", rs: "crates/pxl/web/kernel/src/abi/host.spec.rs", mod: "host" },
  {
    json: "spec/pxl/web/kernel/sched/ledger/ledger.json",
    ts: "apps/web/src/feature/dev/ledger/ledger.spec.ts",
    rs: "crates/pxl/web/kernel/src/sched/ledger/ledger.spec.rs",
    mod: "spec",
    enums: ["CATEGORY_NAMES", "OUTCOME_NAMES", "TASK_PHASE_NAMES", "STEP_NAMES", "EFFECT_NAMES"],
  },
  {
    json: "spec/pxl/web/kernel/mem/mem.json",
    ts: "apps/web/src/kernel/mem/mem.spec.ts",
    rs: "crates/pxl/web/kernel/src/mem/mem.spec.rs",
    mod: "spec",
    tsNames: ["BOOT_AT", "DESKTOP_BYTES", "PHONE_BYTES", "REGION_BYTES", "RING_CAPACITY"],
  },
];

function isLeafWrapper(n: DataNode): n is { $value: DataNode; $doc?: string[]; $type?: string } {
  return typeof n === "object" && !Array.isArray(n) && "$value" in n;
}

function tsLiteral(n: DataNode, type: string, indent: string): string {
  if (isLeafWrapper(n)) return tsLiteral(n.$value, (n.$type as string) ?? type, indent);
  if (typeof n === "number") return `${n} as number`;
  if (typeof n === "boolean") return String(n);
  if (typeof n === "string") return type === "u64" ? `${n}n` : type === "rgb" ? rgbLiteral(n) : JSON.stringify(n);
  if (Array.isArray(n)) return `[${n.map((e) => tsLiteral(e, type, indent)).join(", ")}]`;
  const inner = indent + "  ";
  const t = (n.$type as unknown as string) ?? type;
  const lines = ["{"];
  for (const [k, v] of Object.entries(n)) {
    if (k.startsWith("$")) continue;
    const doc = typeof v === "object" && !Array.isArray(v) ? (v.$doc as unknown as string[] | undefined) : undefined;
    if (doc) lines.push(...tsDoc(doc, inner).map((l, i) => (i === 0 ? l : l.replace(new RegExp(`^${inner}\\*`), `${inner} *`))));
    const key = /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(k) ? k : JSON.stringify(k);
    lines.push(`${inner}${key}: ${tsLiteral(v, t, inner)},`);
  }
  lines.push(`${indent}}`);
  return lines.join("\n");
}

/** Where the Rust of a data spec reads a name the JSON types a string with:
 * the module the spec is written as, and the top-level names it declares. */
type RsScope = { module: string; top: Set<string>; enums: boolean; enumNames: Set<string> };

/** FNV-1a over UTF-8, 64 bits: xpute_core::kit::hash::fnv1a64_bytes. */
function fnv1a64(text: string): string {
  let h = 0xcbf29ce484222325n;
  for (const b of new TextEncoder().encode(text)) {
    h ^= BigInt(b);
    h = BigInt.asUintN(64, h * 0x100000001b3n);
  }
  return `0x${h.toString(16).padStart(16, "0")}`;
}

/** Whether a `$type` names an enum: a path, or a name this spec declares. */
function enumPath(type: string, scope: RsScope): string | null {
  if (/^(u|i)(8|16|32|64)$|^f(32|64)$|^u64$|^rgb$|^hash$|^str$/.test(type)) return null;
  if (type.includes("::") && !scope.top.has(type.split("::")[0])) return type;
  return scope.top.has(type.split("::")[0]) ? `${scope.module}::${type}` : null;
}

/** A leaf as the Rust holds it: a string is the enum member or the hash its
 * `$type` names, and otherwise the string. */
function rsScalar(n: number | string | boolean, type: string, scope: RsScope): { ty: string; lit: string } {
  if (typeof n === "boolean") return { ty: "bool", lit: String(n) };
  if (typeof n === "string") {
    if (type === "u64") return { ty: "u64", lit: n };
    if (type === "rgb") return { ty: "u32", lit: rgbLiteral(n) };
    if (type === "hash") return { ty: "u64", lit: fnv1a64(n) };
    const path = enumPath(type, scope);
    return path === null ? { ty: "&str", lit: JSON.stringify(n) } : { ty: path, lit: `${path}::${rsIdent(n)}` };
  }
  if (type === "f64" || type === "f32") return { ty: type, lit: Number.isInteger(n) ? `${n}.0` : String(n) };
  return { ty: type, lit: String(n) };
}

/** A closed set of names as the Rust names it, the way cube_type.json's
 * channels are: an enum whose members are the JSON's names, numbered as the
 * JSON numbers them — its own numbers where it gives them, its order where it
 * lists them. */
/** An enum's name: the set's name, singular, as cube_type.json's `Land` and
 * decor.json's `Decor` are. */
function enumName(name: string): string {
  const e = pascal(name);
  return e.endsWith("ies") ? `${e.slice(0, -3)}y` : e.endsWith("s") && !e.endsWith("ss") ? e.slice(0, -1) : e;
}

function rsEnum(name: string, variants: [string, number][], indent: string, out: string[]): void {
  const e = enumName(name);
  out.push(`${indent}#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]`, `${indent}#[repr(u8)]`, `${indent}pub enum ${e} {`);
  for (const [v, n] of variants) out.push(`${indent}    ${rsIdent(v)} = ${n},`);
  out.push(`${indent}}`, "");
  out.push(
    `${indent}impl ${e} {`,
    `${indent}    /// Every member, in the JSON's own order.`,
    `${indent}    pub const ALL: [${e}; ${variants.length}] = [`,
    ...variants.map(([v]) => `${indent}        ${e}::${rsIdent(v)},`),
    `${indent}    ];`,
    "",
  );
  out.push(
    `${indent}    /// The member a number names, or none for a number the set does not`,
    `${indent}    /// give out.`,
    `${indent}    pub fn of(value: u8) -> Option<${e}> {`,
    `${indent}        ${e}::ALL.into_iter().find(|member| *member as u8 == value)`,
    `${indent}    }`,
    `${indent}}`,
  );
}

function rsItems(name: string, n: DataNode, type: string, indent: string, out: string[], scope: RsScope): void {
  const doc = isLeafWrapper(n) || (typeof n === "object" && !Array.isArray(n)) ? ((n as { $doc?: string[] }).$doc) : undefined;
  const lines: string[] = [];
  if (isLeafWrapper(n)) rsLeaf(name, n.$value, n.$type ?? type, indent, lines, scope);
  else if (typeof n !== "object" || Array.isArray(n)) rsLeaf(name, n, type, indent, lines, scope);
  else {
    const t = (n.$type as unknown as string) ?? type;
    const keys = Object.keys(n).filter((k) => !k.startsWith("$"));
    // A catalog: names numbered as decor.json numbers its types — its numbers
    // stay constants, as cube_type.spec.rs keeps both, and are one enum beside them
    // where no two names share one — or a set of entries each an object of its
    // own, numbered in order.
    const numbers = keys.map((k) => n[k]);
    const numbered = scope.enums && keys.length > 0 && /^u(8|16|32)$/.test(t) && numbers.every((v) => typeof v === "number" && v < 256);
    if (numbered && new Set(numbers).size === numbers.length) {
      rsEnum(name, keys.map((k) => [k, n[k] as number]), indent, lines);
      lines.push("");
    } else if (scope.enums && keys.length > 0 && keys.every((k) => typeof n[k] === "object" && !Array.isArray(n[k]) && typeof (n[k] as Record<string, DataNode>).$number === "number")) {
      // A set of entries, each naming its own number as decor.json numbers
      // its kinds; an object whose entries carry none is a structure, not a set.
      rsEnum(name, keys.map((k) => [k, (n[k] as Record<string, DataNode>).$number as number]), indent, lines);
      lines.push("");
    }
    const body: string[] = [];
    for (const k of keys) rsItems(k, n[k], t, indent + "    ", body, scope);
    // Each object's keys are also listed as its `KEYS`, where its closed sets
    // are not enums.
    if (!scope.enums) body.push(`${indent}    pub const KEYS: &[&str] = &[${keys.map((k) => JSON.stringify(k)).join(", ")}];`);
    if (body.length > 0) lines.push(`${indent}pub mod ${rsIdent(name)} {`, ...body, `${indent}}`);
  }
  if (lines.length === 0) return;
  if (doc) out.push(...rsDoc(doc, indent));
  out.push(...lines);
}

const RS_KEYWORDS = new Set([
  "type",
  "move",
  "mod",
  "fn",
  "struct",
  "enum",
  "match",
  "loop",
  "ref",
  "self",
  "super",
  "crate",
  "use",
  "where",
  "impl",
  "trait",
  "static",
  "const",
  "let",
  "mut",
  "in",
  "for",
  "if",
  "else",
  "while",
  "return",
  "true",
  "false",
  "as",
  "box",
  "dyn",
  "pub",
  "unsafe",
  "async",
  "await",
  "break",
  "continue",
  "extern",
  "priv",
  "virtual",
  "yield",
  "try",
  "macro",
  "abstract",
  "become",
  "do",
  "final",
  "override",
  "typeof",
  "unsized",
]);

function rsIdent(k: string): string {
  const id = k.replace(/[^A-Za-z0-9_]/g, "_");
  return RS_KEYWORDS.has(id) ? `r#${id}` : /^[0-9]/.test(id) ? `_${id}` : id;
}

function rsLeaf(name: string, v: DataNode, type: string, indent: string, out: string[], scope: RsScope): void {
  if (Array.isArray(v)) {
    if (v.length > 0 && typeof v[0] === "string" && type !== "u64" && enumPath(type, scope) === null && type !== "rgb" && type !== "hash") {
      out.push(`${indent}pub const ${rsIdent(name)}: &[&str] = &[${v.map((e) => JSON.stringify(e)).join(", ")}];`);
      // A list of names is a closed set, and an enum beside it.
      if ((scope.enums || scope.enumNames.has(name)) && v.every((e) => typeof e === "string" && /^[A-Za-z_][\w-]*$/.test(e))) rsEnum(name, (v as string[]).map((e, i) => [e, i]), indent, out);
      return;
    }
    const items = v.map((e) => rsScalar(e as number | string | boolean, type, scope));
    const ty = items[0]?.ty ?? (type === "str" ? "&str" : type);
    out.push(`${indent}pub const ${rsIdent(name)}: [${ty}; ${v.length}] = [${items.map((i) => i.lit).join(", ")}];`);
    return;
  }
  const { ty, lit } = rsScalar(v as number | string | boolean, type, scope);
  out.push(`${indent}pub const ${rsIdent(name)}: ${ty} = ${lit};`);
}

async function dataOutputs(): Promise<Output[]> {
  const outputs: Output[] = [];
  for (const d of DATA_SPECS) {
    const spec = await readJson<DataSpec>(d.json);
    const base = d.json.split("/").pop()!;
    const names = Object.keys(spec).filter((k) => !k.startsWith("$"));
    if (d.ts) {
      const logical = d.ts.replace(/^apps\/web\/src\//, "@pxl/web/");
      const ts = [`// ${logical}`, "//", `// GENERATED by script/spec.ts from ${d.json} — do not edit.`];
      if (spec.$doc) ts.push("//", ...spec.$doc.map((l) => (l === "" ? "//" : `// ${l}`)));
      const body: string[] = [];
      let usesWord = new Set<string>();
      const unknown = (d.tsNames ?? []).filter((n) => !names.includes(n));
      if (unknown.length > 0) throw new Error(`${d.json}: tsNames lists ${unknown.join(", ")}, which the table does not have`);
      for (const name of d.tsNames ? names.filter((n) => d.tsNames!.includes(n)) : names) {
        const n = spec[name];
        body.push("");
        const doc = typeof n === "object" && !Array.isArray(n) ? (n as { $doc?: string[] }).$doc : undefined;
        if (doc) body.push(...tsDoc(doc, "").map((l, i) => (i === 0 ? l : l.replace(/^\*/, " *"))));
        if (isLeafWrapper(n) && typeof n.$value !== "object") {
          const t = n.$type ?? "f64";
          const annot = typeof n.$value === "number" || t === "u64" ? t : null;
          if (annot && annot !== "u64") usesWord.add(annot);
          body.push(`export const ${name}${annot && annot !== "u64" ? `: ${annot}` : ""} = ${tsLiteral(n, t, "")};`);
        } else {
          body.push(`export const ${name} = ${tsLiteral(n, "f64", "")} as const;`);
          // A set numbered by its entries' `$number`, as the Rust enum is:
          // its members' numbers, the way decor.spec.ts lists decor_types.
          const entries = typeof n === "object" && !Array.isArray(n) ? Object.entries(n).filter(([k]) => !k.startsWith("$")) : [];
          if (d.enums === true && entries.length > 0 && entries.every(([, v]) => typeof v === "object" && !Array.isArray(v) && typeof (v as Record<string, DataNode>).$number === "number")) {
            body.push("", `export const ${enumName(name)} = {`, ...entries.map(([k, v]) => `  ${k}: ${(v as Record<string, DataNode>).$number},`), "} as const;");
          }
        }
      }
      if (usesWord.size > 0) ts.push("", `import type { ${[...usesWord].sort().join(", ")} } from "@xpute/core/abi/word.ts";`);
      usesWord = new Set();
      outputs.push({ path: d.ts, text: [...ts, ...body].join("\n") + "\n" });
    }
    if (d.rs) {
      const cratePath = d.rs.replace(/^crates\/pxl\/web\/(kernel|render)\/src\//, "pxl-$1/");
      const rs = [`// ${cratePath}`, "//", `// GENERATED by script/spec.ts from ${base} — do not edit.`];
      if (spec.$doc) rs.push("//", ...spec.$doc.map((l) => (l === "" ? "//" : `// ${l}`)));
      rs.push(
        "",
        ...RS_CASING_NOTE,
        "//",
        "// `dead_code` because a table is generated whole: an entry earns its",
        "// place in the JSON, not by having a caller in this crate.",
        `#![allow(non_upper_case_globals, non_snake_case, ${d.enums ? "non_camel_case_types, " : ""}dead_code)]`,
      );
      const scope: RsScope = {
        module: d.rs.replace(/^crates\/pxl\/web\/(kernel|render)\/src\//, "crate::").replace(/\/[^/]+$/, "").replaceAll("/", "::") + "::" +
          (d.mod ?? d.rs.replace(/^.*\//, "").replace(/\.spec\.rs$/, "")),
        top: new Set(names.map((k) => enumName(k)).concat(names)),
        enums: d.enums === true,
        enumNames: new Set(Array.isArray(d.enums) ? d.enums : []),
      };
      for (const name of names) {
        const items: string[] = [];
        rsItems(name, spec[name], "f64", "", items, scope);
        if (items.length > 0) rs.push("", ...items);
      }
      outputs.push({ path: d.rs, text: rs.join("\n") + "\n" });
    }
  }
  return outputs;
}

// ---- driver ----

// ---- the address space ----
//
// One JSON for every root and branch, written into the three places that build
// a URL or read one — the client's shell, its kernel, and the Worker — and
// checked against the two that can only carry literals: wrangler's own
// `run_worker_first`, and the deploy guard's sentinels. Five copies of one
// decision was what this replaced, and the split that made them five was
// exactly the shape issue/20260910-one-server-root.md was written about.

interface RouteSpec {
  $doc: string[];
  roots: Record<string, { path: string; bundle?: "shell" | "kernel"; $doc?: string[] }>;
  branches: Record<string, { under: string; name: string; bundle?: "shell" | "kernel"; $doc?: string[] }>;
}

async function routeOutputs(): Promise<Output[]> {
  const spec: RouteSpec = JSON.parse(await Deno.readTextFile(join(root, "spec/pxl/spec/route.json")));

  const paths = new Map<string, string>();
  for (const [name, r] of Object.entries(spec.roots)) paths.set(name, r.path);
  for (const [name, b] of Object.entries(spec.branches)) {
    const base = paths.get(b.under);
    if (base === undefined) throw new Error(`spec/pxl/spec/route.json: ${name} is under ${b.under}, which is not a root`);
    paths.set(name, `${base}/${b.name}`);
  }

  const docs = new Map<string, string[]>();
  for (const [name, r] of Object.entries(spec.roots)) if (r.$doc) docs.set(name, r.$doc);
  for (const [name, b] of Object.entries(spec.branches)) if (b.$doc) docs.set(name, b.$doc);

  // Every root is one entry in the edge's own list, which is what keeps a
  // relay reading a prefix rather than a route.
  const rootNames = Object.keys(spec.roots);
  const workerFirst = rootNames.map((n) => `${paths.get(n)}/*`);

  // ---- what only carries literals, checked rather than written ----
  const wrangler = await Deno.readTextFile(join(root, "apps/web/wrangler.jsonc"));
  const declared = wrangler.match(/"run_worker_first"\s*:\s*\[([^\]]*)\]/);
  if (!declared) throw new Error("apps/web/wrangler.jsonc: no run_worker_first to check against spec/pxl/spec/route.json");
  const listed = [...declared[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]).sort();
  if (JSON.stringify(listed) !== JSON.stringify([...workerFirst].sort())) {
    throw new Error(
      `apps/web/wrangler.jsonc's run_worker_first is ${JSON.stringify(listed)} where spec/pxl/spec/route.json's roots are ` +
        `${JSON.stringify(workerFirst)}. A root the edge does not claim is answered by the static-asset fallback with index.html, ` +
        `which reads as the app loading rather than as a route that is not there.`,
    );
  }

  // A root the client only requests under never reaches an artifact as its
  // own literal — a branch is folded into one constant here, so the client
  // carries `/geo/search` and nothing spelling `/geo` alone — and a guard that
  // looked for one would fail every build. What it can look for is the roots
  // and branches the client requests whole, which is what `bundle` marks, in
  // the artifact `bundle` names. Both lists are checked, because a path that
  // moves between the two looks exactly like a path that has gone missing
  // (2026-09-23).
  const guard = await Deno.readTextFile(join(root, "apps/web/script/check-bundle.ts"));
  for (const [list, where] of [["SENTINELS", "shell"], ["KERNEL_ROUTE_SENTINELS", "kernel"]] as const) {
    const found = guard.match(new RegExp(`const ${list} = \\[([^\\]]*)\\]`));
    if (!found) throw new Error(`apps/web/script/check-bundle.ts: no ${list} to check against spec/pxl/spec/route.json`);
    const listed = [...found[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]).sort();
    const want = [...Object.entries(spec.roots), ...Object.entries(spec.branches)].filter(([, r]) => r.bundle === where).map(([n]) => paths.get(n)!).sort();
    if (JSON.stringify(listed) !== JSON.stringify(want)) {
      throw new Error(
        `apps/web/script/check-bundle.ts's ${list} is ${JSON.stringify(listed)} where spec/pxl/spec/route.json marks ` +
          `${JSON.stringify(want)} as reaching the ${where}. A sentinel the build never emits fails every deploy; ` +
          `one the build dropped passes a guard that is checking nothing.`,
      );
    }
  }

  const named = (name: string, value: string) => ({ name, value });
  const all = [
    ...rootNames.map((n) => named(n, paths.get(n)!)),
    ...Object.keys(spec.branches).map((n) => named(n, paths.get(n)!)),
    // The edge and the dev proxy match on a prefix, which is the root or
    // branch with its separator — never a bare root, which would also match a
    // sibling whose name merely starts with it.
    ...rootNames.map((n) => named(`${n.replace(/_ROOT$/, "")}_PREFIX`, `${paths.get(n)}/`)),
  ];

  const head = (path: string, from: string) => [`// ${path}`, "//", `// GENERATED by script/spec.ts from ${from} — do not edit.`, ...spec.$doc.map((l) => (l === "" ? "//" : `// ${l}`)), ""];

  const rs = [
    ...head("pxl-spec/route.spec.rs", "spec/pxl/spec/route.json"),
    ...all.flatMap(({ name, value }) => [...(docs.get(name) ?? []).map((l) => (l === "" ? "///" : `/// ${l}`)), `pub const ${name}: &str = ${JSON.stringify(value)};`, ""]),
    `/// Every root, as the edge claims them.`,
    `pub const WORKER_FIRST: [&str; ${workerFirst.length}] = [${workerFirst.map((w) => JSON.stringify(w)).join(", ")}];`,
    "",
  ].join("\n");

  const ts = [
    ...head("@pxl/web/config/route.spec.ts", "spec/pxl/spec/route.json"),
    ...all.flatMap(({ name, value }) => [...tsDoc(docs.get(name) ?? [], ""), `export const ${name} = ${JSON.stringify(value)};`, ""]),
  ].join("\n");

  const worker = [
    ...head("@pxl/web/cloudflare-worker/route.spec.ts", "spec/pxl/spec/route.json"),
    "// The Worker is bundled for the Workers runtime rather than from src/, so it",
    "// cannot import the shell's copy — but it can import this one, which sits",
    "// beside it and is written from the same JSON. It relays by prefix, so the",
    "// prefixes are all it is given.",
    "",
    ...all.filter(({ name }) => name.endsWith("_PREFIX")).map(({ name, value }) => `export const ${name} = ${JSON.stringify(value)};`),
    "",
  ].join("\n");

  return [
    { path: "crates/pxl/spec/src/route.spec.rs", text: rs },
    { path: "apps/web/src/config/route.spec.ts", text: ts },
    { path: "apps/web/cloudflare-worker/route.spec.ts", text: worker },
  ];
}

// ---- the settings ----
//
// A setting is numbered by its place in SETTINGS, and that number is what
// crosses and what the kernel holds; its name is storage's and the sheet's.
// The kernel gets the settings as one struct of typed fields, with the
// number's reading and writing beside it, so nothing past the store names a
// setting by text. A `dev` setting's name, and its options' names, are under
// the kernel's `dev` feature: a module built without it holds the setting
// at its default and carries none of its words. The shell gets the rows the
// sheet draws, and the same rows without the `dev` ones for a production
// bundle to carry in their place: a filter on the whole table at run time
// leaves every name in the artifact, and a literal the build does not read is
// one the bundler drops.

interface SettingSpec {
  $doc?: string[];
  kind: "toggle" | "choice" | "number";
  default: DataNode;
  section: string;
  cost?: string;
  costs?: string[];
  options?: string;
  dev?: boolean;
  sources_chunks?: boolean;
}

async function settingOutputs(): Promise<Output[]> {
  const json = "spec/pxl/web/kernel/shell/setting/schema.json";
  const spec = await readJson<DataSpec>(json);
  const settings = Object.entries(spec.SETTINGS as unknown as Record<string, SettingSpec>).filter(([k]) => !k.startsWith("$"));
  const lists = new Map<string, { values: string[]; doc?: string[] }>();
  for (const [name, n] of Object.entries(spec)) {
    if (name.startsWith("$") || name === "SETTINGS") continue;
    if (!isLeafWrapper(n) || !Array.isArray(n.$value)) throw new Error(`${json}: ${name} is not a list`);
    lists.set(name, { values: n.$value as string[], doc: n.$doc });
  }
  const optionsOf = (key: string, s: SettingSpec) => {
    const list = s.options === undefined ? undefined : lists.get(s.options);
    if (!list) throw new Error(`${json}: ${key}'s options name no list`);
    return list.values;
  };
  const defaultOf = (n: DataNode) => (isLeafWrapper(n) ? n.$value : n);
  const choices = [...new Set(settings.filter(([, s]) => s.kind === "choice").map(([, s]) => s.options!))];
  const dev = (s: SettingSpec) => s.dev === true;
  const cfg = '#[cfg(feature = "dev")]';

  // ---- Rust ----
  const rs = ["// pxl-kernel/shell/setting/schema.spec.rs", "//", `// GENERATED by script/spec.ts from ${json.split("/").pop()} — do not edit.`, "//"];
  rs.push(...(spec.$doc ?? []).map((l) => (l === "" ? "//" : `// ${l}`)), "", ...RS_CASING_NOTE, "#![allow(non_camel_case_types)]");
  for (const name of choices) {
    const { values, doc } = lists.get(name)!;
    const e = enumName(name);
    rs.push("", ...rsDoc(doc ?? [], ""), "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "#[repr(u32)]", `pub enum ${e} {`, ...values.map((v, i) => `    ${rsIdent(v)} = ${i},`), "}", "");
    rs.push(
      `impl ${e} {`,
      `    pub const ALL: [${e}; ${values.length}] = [${values.map((v) => `${e}::${rsIdent(v)}`).join(", ")}];`,
      "",
      "    /// The option an index names, or none for an index past the list.",
      `    pub fn of(index: u32) -> Option<${e}> {`,
      `        ${e}::ALL.get(index as usize).copied()`,
      "    }",
      "",
      "    /// The name storage keeps it under.",
      "    pub fn name(self) -> &'static str {",
      "        match self {",
      ...values.map((v) => `            ${e}::${rsIdent(v)} => ${JSON.stringify(v)},`),
      "        }",
      "    }",
      "}",
    );
  }
  rs.push("", "/// How a setting is held.", "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "pub enum SettingKind {", "    Toggle,", "    Choice,", "    Number,", "}");
  rs.push("", "/// Every setting, by its place in schema.json's SETTINGS.", "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "#[repr(u32)]", "pub enum SettingKey {");
  settings.forEach(([k, s], i) => rs.push(...rsDoc(s.$doc ?? [], "    "), `    ${k} = ${i},`));
  rs.push("}", "");
  const keysWhere = (test: (s: SettingSpec) => boolean) => {
    const keys = settings.filter(([, s]) => test(s)).map(([k]) => `SettingKey::${k}`);
    return keys.length === 0 ? "false" : `matches!(self, ${keys.join(" | ")})`;
  };
  rs.push(
    "impl SettingKey {",
    `    pub const ALL: [SettingKey; ${settings.length}] = [${settings.map(([k]) => `SettingKey::${k}`).join(", ")}];`,
    "",
    "    /// The setting a number names, or none for a number past the table.",
    "    pub fn of(number: u32) -> Option<SettingKey> {",
    "        SettingKey::ALL.get(number as usize).copied()",
    "    }",
    "",
    "    pub fn kind(self) -> SettingKind {",
    "        match self {",
    ...settings.map(([k, s]) => `            SettingKey::${k} => SettingKind::${pascal(s.kind)},`),
    "        }",
    "    }",
    "",
    "    /// Offered by a development build only.",
    "    pub fn dev(self) -> bool {",
    `        ${keysWhere(dev)}`,
    "    }",
    "",
    "    /// Read by a chunk's build, so a change builds the chunks again.",
    "    pub fn sources_chunks(self) -> bool {",
    `        ${keysWhere((s) => s.sources_chunks === true)}`,
    "    }",
    "",
    "    /// The name storage keeps it under, or none where this module does not",
    "    /// name it.",
    "    pub fn name(self) -> Option<&'static str> {",
    "        match self {",
    ...settings.flatMap(([k, s]) => [...(dev(s) ? [`            ${cfg}`] : []), `            SettingKey::${k} => Some(${JSON.stringify(k)}),`]),
    ...(settings.some(([, s]) => dev(s)) ? ["            #[allow(unreachable_patterns)]", "            _ => None,"] : []),
    "        }",
    "    }",
    "",
    "    /// A choice's option by its index, as storage names it.",
    "    pub fn option_name(self, index: u32) -> Option<&'static str> {",
    "        match self {",
    ...settings.filter(([, s]) => s.kind === "choice").flatMap((
      [k, s],
    ) => [...(dev(s) ? [`            ${cfg}`] : []), `            SettingKey::${k} => ${enumName(s.options!)}::of(index).map(${enumName(s.options!)}::name),`]),
    "            _ => None,",
    "        }",
    "    }",
    "",
    "    /// A choice's option by the name storage keeps it under, as its index.",
    "    pub fn option_index(self, name: &str) -> Option<u32> {",
    "        match self {",
    ...settings.filter(([, s]) => s.kind === "choice").flatMap(([k, s]) => [
      ...(dev(s) ? [`            ${cfg}`] : []),
      `            SettingKey::${k} => ${enumName(s.options!)}::ALL.into_iter().find(|o| o.name() == name).map(|o| o as u32),`,
    ]),
    "            _ => None,",
    "        }",
    "    }",
    "}",
  );
  const fieldType = (k: string, s: SettingSpec) => {
    if (s.kind === "toggle") return "bool";
    if (s.kind === "choice") return enumName(s.options!);
    const d = s.default;
    if (!isLeafWrapper(d) || !/^u(8|16|32)$/.test(d.$type ?? "")) throw new Error(`${json}: ${k} is a number with no width`);
    return d.$type!;
  };
  const defaultLit = (k: string, s: SettingSpec) => {
    const d = defaultOf(s.default);
    if (s.kind === "choice") {
      const at = optionsOf(k, s).indexOf(d as string);
      if (at < 0) throw new Error(`${json}: ${k}'s default is not one of its options`);
      return `${enumName(s.options!)}::${rsIdent(d as string)}`;
    }
    return String(d);
  };
  rs.push("", "/// Every setting's value: what the store holds, and what the app reads.", "#[derive(Clone, Debug, PartialEq)]", "pub struct Settings {");
  for (const [k, s] of settings) rs.push(`    pub ${k}: ${fieldType(k, s)},`);
  rs.push("}", "", "impl Settings {", "    /// Every setting at schema.json's default.", "    pub const DEFAULT: Settings = Settings {");
  for (const [k, s] of settings) rs.push(`        ${k}: ${defaultLit(k, s)},`);
  rs.push("    };", "");
  rs.push(
    "    /// A setting's value as it crosses: a toggle's 0 or 1, a choice's option",
    "    /// by its index, a number.",
    "    pub fn get(&self, key: SettingKey) -> u32 {",
    "        match key {",
    ...settings.map(([k]) => `            SettingKey::${k} => self.${k} as u32,`),
    "        }",
    "    }",
    "",
    "    /// Sets a setting from its value as it crosses; false, and nothing set,",
    "    /// for a value the setting cannot hold.",
    "    pub fn set(&mut self, key: SettingKey, value: u32) -> bool {",
    "        match key {",
    ...settings.map(([k, s]) => {
      if (s.kind === "toggle") return `            SettingKey::${k} if value <= 1 => self.${k} = value == 1,`;
      if (s.kind === "choice") return `            SettingKey::${k} => match ${enumName(s.options!)}::of(value) { Some(o) => self.${k} = o, None => return false },`;
      return `            SettingKey::${k} => match ${fieldType(k, s)}::try_from(value) { Ok(n) => self.${k} = n, Err(_) => return false },`;
    }),
    "            _ => return false,",
    "        }",
    "        true",
    "    }",
    "}",
  );

  // ---- TypeScript ----
  const row = (k: string, s: SettingSpec, i: number): DataNode => {
    const out: Record<string, DataNode> = { key: { $value: i, $type: "u32" } as unknown as DataNode };
    if (s.$doc) out.$doc = s.$doc as unknown as DataNode;
    for (const [f, v] of Object.entries(s)) {
      if (f === "$doc" || f === "sources_chunks") continue;
      out[f] = f === "options" ? optionsOf(k, s) : f === "default" ? defaultOf(v as DataNode) : (v as DataNode);
    }
    return out as DataNode;
  };
  const table = (keep: (s: SettingSpec) => boolean) => Object.fromEntries(settings.map(([k, s], i) => [k, s, i] as const).filter(([, s]) => keep(s)).map(([k, s, i]) => [k, row(k, s, i)]));
  const ts = [`// @pxl/web/feature/setting/schema.spec.ts`, "//", `// GENERATED by script/spec.ts from ${json} — do not edit.`, "//", ...(spec.$doc ?? []).map((l) => (l === "" ? "//" : `// ${l}`))];
  ts.push("", ...tsDoc(lists.get("DEV_SECTIONS")!.doc ?? [], ""), `export const DEV_SECTIONS = ${tsLiteral(lists.get("DEV_SECTIONS")!.values, "f64", "")} as const;`);
  ts.push("", `export const SETTINGS = ${tsLiteral(table(() => true), "f64", "")} as const;`);
  ts.push("", "/** SETTINGS without its `dev` entries: what a production bundle carries in its place. */", `export const SETTINGS_SHIPPED = ${tsLiteral(table((s) => !dev(s)), "f64", "")} as const;`);

  return [
    { path: "crates/pxl/web/kernel/src/shell/setting/schema.spec.rs", text: rs.join("\n") + "\n" },
    { path: "apps/web/src/feature/setting/schema.spec.ts", text: ts.join("\n") + "\n" },
  ];
}

const GENERATORS: (() => Promise<Output[]>)[] = [
  cmdOutputs,
  settingOutputs,
  errnoOutputs,
  fetchOutputs,
  colorOutputs,
  cubeTypeOutputs,
  decorOutputs,
  publishedOutputs,
  poiThemeOutputs,
  dataOutputs,
  routeOutputs,
];

/** An output as `cargo fmt` or `deno fmt` leaves it, so formatting the tree
 * does not make a generated file stale. */
async function formatted(o: Output): Promise<Output> {
  const rust = o.path.endsWith(".rs");
  if (!rust && !o.path.endsWith(".ts")) return o;
  const command = rust
    ? new Deno.Command("rustfmt", { args: ["--edition", "2021", "--emit", "stdout", "--config-path", join(root, "rustfmt.toml")], stdin: "piped", stdout: "piped", stderr: "piped" })
    : new Deno.Command(Deno.execPath(), { args: ["fmt", "--ext", "ts", "-"], cwd: root, stdin: "piped", stdout: "piped", stderr: "piped" });
  const child = command.spawn();
  const writer = child.stdin.getWriter();
  await writer.write(new TextEncoder().encode(o.text));
  await writer.close();
  const { code, stdout, stderr } = await child.output();
  if (code !== 0) throw new Error(`fmt ${o.path}: ${new TextDecoder().decode(stderr)}`);
  return { path: o.path, text: new TextDecoder().decode(stdout) };
}

const outputs = await Promise.all((await Promise.all(GENERATORS.map((g) => g()))).flat().map(formatted));
if (Deno.args.includes("--write")) {
  for (const o of outputs) {
    await Deno.mkdir(dirname(join(root, o.path)), { recursive: true });
    await Deno.writeTextFile(join(root, o.path), o.text);
    console.log(`wrote ${o.path}`);
  }
} else {
  const stale: string[] = [];
  for (const o of outputs) {
    let onDisk: string | null = null;
    try {
      onDisk = await Deno.readTextFile(join(root, o.path));
    } catch { /* missing */ }
    if (onDisk !== o.text) stale.push(o.path);
  }
  if (stale.length > 0) {
    for (const s of stale) console.error(`spec: ${s} is not what its JSON writes — run \`deno task spec\``);
    Deno.exit(1);
  }
  console.log(`spec: ${outputs.length} generated files are what their JSON writes`);
}
