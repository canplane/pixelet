#!/usr/bin/env -S deno run -A
// Copies xpute out of pixelet, as the repository it will be when it leaves.
//
// pixelet stays the source of truth and the export is a derived artifact, the
// way the build counter and the build log are: one place holds the thing, and
// what leaves is produced rather than maintained. The alternative — developing
// xpute in its own repository and consuming it by version — costs a publish
// and a bump for every change, and the boundary is still moving (38 of the 49
// commits that touched xpute in its first month landed in one week).
//
// **The layout is preserved exactly.** `crates/xpute/`, `packages/xpute/` and
// `spec/xpute/` keep their paths, which is not cosmetic: `golden.ts` reaches
// the Rust tree's records at `../../../../crates/xpute/core/golden/`, because
// one copy read from both sides is the only arrangement that cannot drift
// silently. Rewriting a path to flatten the tree would reintroduce exactly
// what that arrangement avoids — and the day xpute really leaves, nothing has
// to change.
//
// So **no source file is edited**. Every `.rs`, `.ts`, `.tsv` and `.json`
// under the three trees is copied byte for byte, listed by `git ls-files` so
// that no build output can come along. Four manifests are written instead of
// copied, because pixelet's own say things that are not xpute's: the Cargo
// workspace (whose members and header describe pxl), `deno.jsonc` (28 tasks,
// all of them pixelet's), the notices (shadcn is the shell's) and the licence
// — pixelet's is proprietary and xpute is MIT.
//
// The export carries the commit it came from. It cannot stop someone from
// letting the copy go stale, so it makes staleness legible instead: `--check`
// answers whether the two agree, and the stamp in README says what it was
// taken from.
//
// Usage (from the repo root):
//   deno task export:xpute --to ../xpute
//   deno task export:xpute --to ../xpute --check

import { dirname, join, relative } from "@std/path";

const ROOT = dirname(dirname(new URL(import.meta.url).pathname));

/** The trees that are xpute's, and the root files that belong with them. */
const TREES = ["crates/xpute", "packages/xpute", "spec/xpute"];
const ROOT_FILES = ["rustfmt.toml", "ARCHITECTURE.xpute.md"];

/** Where a copied file lands under a different name. The `.xpute` infix tells
 * two documents apart in pixelet, where there are two; here there is one, and
 * the infix would name the other repository's arrangement in a tree that has
 * no second document to be told apart from. */
const RENAMED: Record<string, string> = { "ARCHITECTURE.xpute.md": "ARCHITECTURE.md" };

/** Names an earlier export wrote and this one does not. Leaving one behind is
 * not inert: where both are present Deno reads `deno.json` and never looks at
 * `deno.jsonc`, so the stale copy would go on being the repository's tasks. */
const RETIRED = ["deno.json"];

/** The Cargo workspace members, in the order the source manifest lists them. */
const CRATES = ["xpute/core", "xpute/runtime"];

/** What an export carries, for the purpose of saying what it carries: the
 * three trees and the two files that are copied, and this script, which is
 * not copied but decides four of the files that are. */
const CARRIED = [...TREES, ...ROOT_FILES, "script/export-xpute.ts"];

/** What the Rust half actually depends on. The versions and the reasons are
 * read out of pixelet's manifest rather than repeated here, so a re-pin
 * upstream cannot leave the export behind. */
const RUST_DEPS = ["libm", "fnv", "indexmap", "hex"];

/** What the export does not manage, and so neither writes nor objects to:
 * build output, and the lock files, which belong to whoever runs the gate
 * there. Two libraries' locks bind no consumer, and pixelet's own are the
 * ones its builds are held to. */
const UNMANAGED = ["crates/target/", "node_modules/", "crates/Cargo.lock", "deno.lock"];

const unmanaged = (path: string) => UNMANAGED.some((u) => (u.endsWith("/") ? path.startsWith(u) || path.includes(`/${u}`) : path === u));

function git(args: string[]): string {
  const out = new Deno.Command("git", { args, cwd: ROOT, stdout: "piped", stderr: "piped" }).outputSync();
  if (!out.success) die(`git ${args.join(" ")}: ${new TextDecoder().decode(out.stderr).trim()}`);
  return new TextDecoder().decode(out.stdout).trimEnd();
}

function die(message: string): never {
  note(message);
  Deno.exit(1);
}

/** Progress and findings go to stderr, so that stdout carries the commit
 * message alone and can be piped into `git commit -F -`. */
function note(message: string): void {
  console.error(`[export-xpute] ${message}`);
}

/** Whether a commit is still here. Asked rather than assumed, because the
 * history a previous export named may have been rewritten since — and `git`
 * above dies on a failure, which this one is not. */
function resolves(sha: string): boolean {
  return new Deno.Command("git", { args: ["cat-file", "-e", `${sha}^{commit}`], cwd: ROOT, stdout: "null", stderr: "null" }).outputSync().success;
}

/** Which commit this export was taken from, `-dirty` when the tree held
 * uncommitted changes — the same claim `build_id()` makes about a build, and
 * for the same reason: a name that does not resolve back to these bytes is
 * worth saying out loud. */
function stamp(): string {
  // What the export is made of and not the whole repository: work in flight
  // elsewhere in pixelet says nothing about whether checking out this commit
  // gives these bytes. This script counts, though it is not copied — four of
  // the exported files are what it writes.
  const dirty = git(["status", "--porcelain", "--", ...CARRIED]) !== "" ? "-dirty" : "";
  return `pixelet@${git(["rev-parse", "--short", "HEAD"])}${dirty}`;
}

// ---- the four manifests, written rather than copied ----

/** A `[section]` of a TOML file, from its header to the next one, with the
 * comment block above the header. */
function section(toml: string, header: string): string {
  const lines = toml.split("\n");
  const at = lines.findIndex((l) => l.trim() === header);
  if (at < 0) die(`${header} is not in crates/Cargo.toml any more`);
  let from = at;
  while (from > 0 && lines[from - 1].startsWith("#")) from--;
  let to = at + 1;
  while (to < lines.length && !lines[to].startsWith("[")) to++;
  return lines.slice(from, to).join("\n").trimEnd();
}

/** One dependency's line and the comment block above it, so that the reason a
 * version is pinned travels with the pin. */
function dependency(toml: string, name: string): string {
  const lines = section(toml, "[workspace.dependencies]").split("\n");
  const at = lines.findIndex((l) => l.startsWith(`${name} = `) || l.startsWith(`${name}.`) || l.startsWith(`${name} =`));
  if (at < 0) die(`${name} is not in [workspace.dependencies] any more`);
  let from = at;
  while (from > 0 && lines[from - 1].startsWith("#")) from--;
  return lines.slice(from, at + 1).join("\n");
}

function cargoManifest(): string {
  const source = Deno.readTextFileSync(join(ROOT, "crates/Cargo.toml"));
  return `# The Rust half of the kit, and the runtime over it. \`core\` is what any
# program that links it may use; \`runtime\` asks to be run its way — a turn on
# a doorbell, a quota measured from the frame, I/O granted by credits — and
# depends on \`core\`, never the reverse.
#
# GENERATED by pixelet's script/export-xpute.ts. Its dependency versions and
# the reasons for them are pixelet's own, copied with the pin.

[workspace]
resolver = "2"
members = [
${CRATES.map((c) => `  "${c}",`).join("\n")}
]

[workspace.dependencies]
${RUST_DEPS.map((d) => dependency(source, d)).join("\n")}

${section(source, "[workspace.package]")}
`;
}

function denoManifest(): string {
  return `{
  // GENERATED by pixelet's script/export-xpute.ts.

  "description": "A kit and a runtime for a guest program inside a host.",

  "workspace": ["packages/xpute/core", "packages/xpute/runtime"],
  "nodeModulesDir": "auto",

  "tasks": {
    "check": "deno fmt --check && deno check packages/xpute/core/src/**/*.ts && deno check packages/xpute/runtime/src/**/*.ts && cargo fmt --manifest-path crates/Cargo.toml --all --check && cargo clippy --manifest-path crates/Cargo.toml --all-targets -- -D warnings",
    "test": "deno test -A packages/xpute/core/src && deno test -A packages/xpute/runtime/src && cargo test --manifest-path crates/Cargo.toml --all",
    "fmt": "deno fmt && cargo fmt --manifest-path crates/Cargo.toml --all",
    "lint": "deno lint"
  },

  "compilerOptions": {
    "lib": ["deno.window", "deno.ns", "deno.unstable", "esnext"],
    "verbatimModuleSyntax": true,
    "noImplicitOverride": true,
    "skipLibCheck": true,
    "checkJs": false
  },

  "imports": {
    "@xpute/core/": "./packages/xpute/core/src/",
    "@xpute/runtime/": "./packages/xpute/runtime/src/",
    "@std/assert": "jsr:@std/assert@^1",
    "@std/path": "jsr:@std/path@^1"
  },

  "fmt": {
    "lineWidth": 200,
    "proseWrap": "preserve",

    // The golden records are committed as they were written and read byte
    // for byte by the tests; formatting them would break that.
    "exclude": ["node_modules/", "crates/**/golden/"]
  },

  "lint": { "rules": { "tags": ["recommended"] }, "exclude": ["node_modules/"] }
}
`;
}

function license(): string {
  return `MIT License

Copyright (c) 2025 canplane

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
`;
}

/** The one file here that is someone else's, and the file that says so. A
 * notice naming a path that is no longer exported would be a claim with
 * nothing behind it, so the export refuses rather than shipping one. */
const PORTED = "crates/xpute/core/src/alloc/buddy_tree.rs";

function notices(): string {
  return `# Third-party notices

One file here is a port of someone else's code rather than our own, and its
licence asks for a notice to travel with the copy. It carries that notice
inside itself; this records where it came from.

This file covers copied source only. Crates and packages that are merely
depended on are not listed: \`crates/Cargo.toml\` and the package manifests name
them, and each carries its own licence where it is published.

## evanw/buddy-malloc — \`${PORTED}\`

That file is [evanw/buddy-malloc][buddy] (MIT) line for line, and the notice
is at the top of it.

[buddy]: https://github.com/evanw/buddy-malloc
`;
}

/** The sections of pixelet's own `.gitignore` that apply to a repository with
 * no `apps/web`, no bake output and no bundler — taken by name rather than
 * copied as text, so that a tool added upstream is ignored here too. */
const IGNORE_SECTIONS = [
  "# General OS files",
  "# Logs",
  "# Dependency directories",
  "# AI coding tools",
  "# Personal, uncommitted variants of shared agent content",
  "# Backup files sometimes created by rustfmt/editor tooling",
];

/** A block of a `.gitignore`, from its header comment to the blank line after
 * it. Comment lines directly under the header belong to it. */
function ignoreSection(text: string, header: string): string {
  const lines = text.split("\n");
  const at = lines.findIndex((l) => l === header);
  if (at < 0) die(`${JSON.stringify(header)} is not in .gitignore any more`);
  let to = at;
  while (to < lines.length && lines[to].trim() !== "") to++;
  return lines.slice(at, to).join("\n");
}

function gitignore(): string {
  const source = Deno.readTextFileSync(join(ROOT, ".gitignore"));
  return `${IGNORE_SECTIONS.map((h) => ignoreSection(source, h)).join("\n\n")}

# Rust workspace build output
/crates/target

# Written by whoever runs the gate here, and not by the export. Two libraries'
# locks bind no consumer, and pixelet's own are what its builds are held to.
/crates/Cargo.lock
/deno.lock
`;
}

function readme(at: string): string {
  return `# xpute

A guest program runs inside a host. The host owns the clock, the memory and the I/O, and grants each of them a slice at a time; the guest computes with what it was granted and asks for the rest. **xpute is both ends of that arrangement, and the kit underneath it** — written twice, once in Rust and once in TypeScript.

The shape it was written for is a Rust program compiled to one WebAssembly module, driven by TypeScript in a browser tab: one side owns a machine the other cannot leave. Nothing in xpute knows what that program computes.

## The arrangement

\`\`\`text
host · TypeScript                          guest · Rust
─────────────────                          ────────────

strobe   a frame, and only when one
         was asked for
quantum  a quota, from the frame it
         measured rather than assumed

commands ─────────── SQ ring ───────────▶  drained on the rising edge
doorbell ═════ interrupt(quota_ms) ═════▶  ┌ tick   phases in a fixed order
                                           │ tasks  futures that yield when
                                           │        the quota is spent
                                           │ queue  what I/O it wants run
completions ◀───────── CQ ring ──────────  └ posted before the falling edge
         ◀═══════════ wake_ms ═══════════  next frame · in n ms · never

credits  runs what that queue started,
         as far as the grant goes

         … nothing until wake_ms …         … nothing at all …
\`\`\`

Each of the host's levers faces one of the guest's. The **strobe** decides whether a turn happens at all, the **tick** what runs inside it. The **quantum** sizes the turn; the **tasks** yield when it is spent. The **credits** run the I/O the **queue** asked for, since the guest runs none. The **doorbell** is the one call in, \`wake_ms\` the one answer out.

The strobe rings on an animation frame, and only when something asked for a turn: an input or an arrival on the host's side, or the guest's own last answer. A guest that wants nothing is rung for nothing. The quota is sized from a frame interval the host measures rather than assumes, and a turn that runs long is charged for it in the next one.

The call itself carries nothing but the quota. Everything else crosses through one memory that both ends read where it lies.

\`\`\`text
┌─ one linear memory ─ reserved once, never grown, read in place ──────────┐
│  sections    ranges laid end to end, placed at boot                      │
│  rings       the SQ and CQ above: four-word frames, payloads apart       │
│  change log  topic · key · revision, the read direction's own channel    │
│  heap        one door, with a slab over a buddy behind it                │
└──────────────────────────────────────────────────────────────────────────┘
\`\`\`

A tick spends a quota of time on the guest's own work; the queue spends a grant of credits on work the host does for it. The host owns the connections the way an operating system owns its devices, and that is the whole of the division.

## \`core\`, and \`runtime\`

\`core\` is a kit and nothing more: an arena, a deque and a binary heap; a JSON codec; scalar, vec3, mat4 and an rng; an errno table; XTP and TLV; and, on the Rust side, a buddy and a slab. Link it and use it — it knows nothing of turns, hosts or memories. It is the same kit on both sides, each with the few pieces only its own language needs, and the golden records under \`crates/xpute/core/golden/\` hold the two to each other wherever bytes cross. The TypeScript tests read those same record files rather than keeping a copy, because a second copy is how two implementations come to disagree without anything failing.

\`runtime\` is the arrangement above, and it is not symmetric — the two halves are the two ends of one thing, not two implementations of it. A module on one side with nothing facing it is not a gap to fill. What the two must agree on is the format: the ring's layout, the frame's four words, the memory's sections.

\`\`\`text
crates/xpute/core     ·  packages/xpute/core       the same kit, twice
crates/xpute/runtime  ·  packages/xpute/runtime    the guest end · the host end
spec/xpute                                         numbers both are generated from
\`\`\`

## What it leaves to whoever uses it

The line is drawn in the same place every time: xpute carries the mechanism, and the policy belongs to the program.

- **It does not know what the guest computes.** No domain types reach \`core\`, which is what keeps it portable to a second program.
- **It does not choose the memory map.** \`mem/section\` is told a base, a unit and a list of sizes, and answers where each range landed. What the ranges hold is the guest's.
- **It does not choose an allocator.** The heap is one door. \`core\` offers a buddy and a slab that fit behind it, and a guest is free to put something else there.
- **It is not a transport.** A ring is a producer, a consumer and a memory they share. What carries bytes to another machine is the host's business.
- **It is not real time.** A quota is how long a turn should take, not a deadline. Nothing preempts: a turn that runs long is charged for it in the next one rather than cut off.
- **It is not an RPC framework.** A message is four words — who asked, what is asked, what came back, where the packet is — and the numbers are the program's own.

[ARCHITECTURE.md](./ARCHITECTURE.md) is what each piece is and why it is shaped that way: the memory, the rings, the change log, the turn, the credits, the handles, the wire formats, the records.

## Running it

Deno 2.x and a Rust toolchain.

\`\`\`sh
deno task check   # deno fmt --check, deno check, cargo fmt --check, clippy
deno task test    # deno test and cargo test, the golden records included
deno task lint    # deno lint
\`\`\`

## Where this comes from

This is a copy. xpute is developed inside pixelet, a closed application that is its only consumer so far, and exported here whole — the layout is pixelet's own, so the day it moves for real nothing has to change. Pull requests are welcome as conversation, but the change has to land upstream.

[plei.me](https://plei.me) is that application running, which is the only public view of this kit at work.

Taken from **${at}**.

The four files generated from \`spec/xpute/\` — the errno table and the fetch ABI, on both sides — are checked in as pixelet generates them. The generator itself knows every one of pixelet's specs and did not come along, so this copy reads but does not regenerate them.

## Licence

MIT ([LICENSE](./LICENSE)). One file under \`alloc/\` is a port rather than a dependency, and [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md) carries what its licence asks.
`;
}

/** The stamp the copy already carries, read before it is overwritten. It is
 * what makes a draft possible: the range from there to here is what this
 * export is carrying that the last one did not. */
function previousStamp(to: string): string | null {
  try {
    return Deno.readTextFileSync(join(to, "README.md")).match(/pixelet@([0-9a-f]+)/)?.[1] ?? null;
  } catch {
    return null;
  }
}

/** What to say about an export: which commit it is, and the subjects of the
 * upstream commits it carries. A mirror's commit has no single kind — it holds
 * whatever happened upstream, a feature and three fixes at once — so it is
 * named by what it is rather than prefixed by what it does.
 *
 * A previous stamp that no longer resolves, because the history it named was
 * rewritten, leaves the stamp alone as the only honest thing to say. */
function draft(to: string, at: string): string {
  const previous = previousStamp(to);
  const reachable = previous !== null && resolves(previous);
  const subjects = reachable ? git(["log", "--format=%s", `${previous}..HEAD`, "--", ...CARRIED]) : "";
  const carried = subjects.split("\n").filter((l) => l !== "");
  if (carried.length === 0) return `${at}\n`;
  return `${at}\n\n${carried.map((s) => `  ${s}`).join("\n")}\n`;
}

// ---- building it ----

type File = { path: string; bytes: Uint8Array };

/** Everything the export holds: the tracked source byte for byte, then the
 * files that are written because pixelet's own say what is not xpute's. */
function build(): File[] {
  const encoder = new TextEncoder();
  const listed = git(["ls-files", "-z", "--", ...TREES, ...ROOT_FILES]).split("\0").filter((p) => p !== "");
  if (listed.length === 0) die("git listed nothing to export");

  if (!listed.includes(PORTED)) die(`${PORTED} is not being exported, and THIRD_PARTY_NOTICES.md names it`);

  const files: File[] = listed.map((path) => ({ path: RENAMED[path] ?? path, bytes: Deno.readFileSync(join(ROOT, path)) }));
  const at = stamp();
  for (
    const [path, text] of [
      ["crates/Cargo.toml", cargoManifest()],
      ["deno.jsonc", denoManifest()],
      ["README.md", readme(at)],
      ["LICENSE", license()],
      ["THIRD_PARTY_NOTICES.md", notices()],
      [".gitignore", gitignore()],
    ] as const
  ) {
    files.push({ path, bytes: encoder.encode(text) });
  }
  return files;
}

/** Every relative link in the markdown, held against what is actually going
 * out. A sentence that is true in pixelet is not true here by itself — a
 * cross-reference to a document this repository does not carry reads as a
 * file it lost rather than one it never had, and the first export shipped two
 * of those. The strings are copied verbatim on purpose, so the check is what
 * catches the ones that stop being true on the way. */
function checkLinks(files: File[]): void {
  const here = new Set(files.map((f) => f.path));
  const decoder = new TextDecoder();
  const broken: string[] = [];
  for (const file of files) {
    if (!file.path.endsWith(".md")) continue;
    const text = decoder.decode(file.bytes);
    for (const [, target] of text.matchAll(/\]\((\.\/[^)#\s]+|[^):#\s]+\.md)[^)]*\)/g)) {
      const at = target.replace(/^\.\//, "");
      const full = file.path.includes("/") ? join(dirname(file.path), at) : at;
      if (!here.has(full)) broken.push(`${file.path} -> ${target}`);
    }
  }
  if (broken.length > 0) {
    for (const one of broken) note(`  broken  ${one}`);
    die("a link points at something this export does not carry");
  }
}

function same(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((byte, i) => byte === b[i]);
}

/** What the export holds that is not the target's, and what the target holds
 * that the export does not. The stamp is left out of the comparison: it moves
 * with every commit, and a copy that is otherwise identical is not stale. */
function compare(files: File[], to: string): { differs: string[]; extra: string[] } {
  const differs: string[] = [];
  for (const file of files) {
    if (file.path === "README.md") continue;
    let held: Uint8Array;
    try {
      held = Deno.readFileSync(join(to, file.path));
    } catch {
      differs.push(file.path);
      continue;
    }
    if (!same(file.bytes, held)) differs.push(file.path);
  }

  const known = new Set(files.map((f) => f.path));
  const extra: string[] = [];
  const walk = (dir: string) => {
    for (const entry of Deno.readDirSync(dir)) {
      if (entry.name === ".git" || unmanaged(`${relative(to, dir)}/${entry.name}/`.replace(/^\//, ""))) continue;
      const full = join(dir, entry.name);
      if (entry.isDirectory) walk(full);
      else if (!known.has(relative(to, full)) && !unmanaged(relative(to, full))) extra.push(relative(to, full));
    }
  };
  try {
    walk(to);
  } catch {
    die(`${to} is not a directory; clone the repository there first`);
  }
  return { differs, extra };
}

function write(files: File[], to: string): void {
  // Only what this script manages is cleared, so anything the repository keeps
  // of its own — `.git`, a workflow, an issue template — survives an export.
  for (const tree of TREES) {
    try {
      Deno.removeSync(join(to, tree), { recursive: true });
    } catch { /* not there yet */ }
  }
  // A file that used to go out under its own name and now goes out under
  // another would otherwise sit beside its replacement for good.
  for (const was of [...Object.keys(RENAMED), ...RETIRED]) {
    try {
      Deno.removeSync(join(to, was));
    } catch { /* never was, or already gone */ }
  }
  for (const file of files) {
    const full = join(to, file.path);
    Deno.mkdirSync(dirname(full), { recursive: true });
    Deno.writeFileSync(full, file.bytes);
  }
}

// ---- the command ----

const args = Deno.args;
const toAt = args.indexOf("--to");
if (toAt < 0 || !args[toAt + 1]) die("--to <dir> is required: the xpute repository to export into");
const to = args[toAt + 1];
const checking = args.includes("--check");

const files = build();
checkLinks(files);

if (checking) {
  const { differs, extra } = compare(files, to);
  if (differs.length === 0 && extra.length === 0) {
    note(`${to} is what this tree holds (${files.length} files)`);
    Deno.exit(0);
  }
  for (const path of differs) note(`  differs  ${path}`);
  for (const path of extra) note(`  extra    ${path}`);
  die(`${to} is not what this tree holds`);
}

const at = stamp();
const message = draft(to, at);
write(files, to);
note(`${files.length} files into ${to}, from ${at}`);
console.log(message);
