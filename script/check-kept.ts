#!/usr/bin/env -S deno run -A
// What the working tree's changes kept under another name or place: a line
// removed that returns with only its identifiers changed, and lines removed
// from one file that return whole in another.
//
//   deno run -A script/check-kept.ts
//
// A heuristic, and a compromise: it compares lines, so a structure rewritten
// in new lines, or a rename that also reflows a line, is not seen, and a line
// whose identifiers changed for another reason is listed. It lists; it
// refuses nothing.

import { dirname, fromFileUrl, join } from "@std/path";

const root = dirname(dirname(fromFileUrl(import.meta.url)));

function git(...args: string[]): string {
  const out = new Deno.Command("git", { args, cwd: root, stdout: "piped" }).outputSync();
  if (!out.success) throw new Error(`git ${args.join(" ")} failed`);
  return new TextDecoder().decode(out.stdout);
}

interface Line {
  file: string;
  at: number;
  text: string;
}

interface Hunk {
  removed: Line[];
  added: Line[];
}

// Generated or not code: their lines repeat by construction.
const skipped = (file: string) => file.includes(".spec.") || /(^|\/)(Cargo|deno|package)\.lock$|-lock\.json$/.test(file);

// --no-renames: a moved file is a deletion and an addition, which the
// cross-file pass pairs.
const hunks: Hunk[] = [];
let file = "";
let at = [0, 0];
let hunk: Hunk | null = null;
for (const line of git("diff", "HEAD", "-U0", "--no-renames", "--no-color").split("\n")) {
  // A file's header runs to its first hunk: a removed line may read `--- `.
  if (line.startsWith("diff --git")) {
    hunk = null;
  } else if (!hunk && line.startsWith("+++ ")) {
    file = line === "+++ /dev/null" ? file : line.slice(6);
  } else if (!hunk && line.startsWith("--- ")) {
    file = line === "--- /dev/null" ? "" : line.slice(6);
  } else if (line.startsWith("@@")) {
    const m = /^@@ -(\d+)(?:,\d+)? \+(\d+)/.exec(line)!;
    at = [Number(m[1]), Number(m[2])];
    hunk = { removed: [], added: [] };
    if (!skipped(file)) hunks.push(hunk);
  } else if (hunk && line.startsWith("-")) {
    hunk.removed.push({ file, at: at[0]++, text: line.slice(1) });
  } else if (hunk && line.startsWith("+")) {
    hunk.added.push({ file, at: at[1]++, text: line.slice(1) });
  }
}
for (const path of git("ls-files", "--others", "--exclude-standard").split("\n").filter(Boolean)) {
  if (skipped(path)) continue;
  let text: string;
  try {
    text = await Deno.readTextFile(join(root, path));
  } catch {
    continue;
  }
  hunks.push({ removed: [], added: text.split("\n").map((t, i) => ({ file: path, at: i + 1, text: t })) });
}

// A line as tokens: words, numbers, and each other character but space.
const tokens = (text: string) => text.match(/[A-Za-z_][A-Za-z0-9_]*|\d[\d_.]*|\S/g) ?? [];
const word = (t: string) => /^[A-Za-z_]/.test(t);
// What a rename never changes: a keyword changed is a different line.
const KEYWORDS = new Set(
  "as async await break const continue crate dyn else enum export extends false fn for from function if impl import in interface let loop match mod move mut new null pub ref return self Self static struct super trait true type undefined unsafe use where while yield"
    .split(" "),
);
const shape = (ts: string[]) => ts.map((t) => (word(t) && !KEYWORDS.has(t) ? "\0" : t)).join(" ");
// Too short to tell one line from another; an import list reorders as its
// names change.
const distinct = (ts: string[]) => ts.filter(word).length >= 3 && !["use", "import"].includes(ts[0]);

/** The words changed between two lines of one shape, old to new; none where nothing is. */
function renamed(a: string[], b: string[]): [string, string][] {
  const out: [string, string][] = [];
  for (let k = 0; k < a.length; k++) if (a[k] !== b[k]) out.push([a[k], b[k]]);
  return out;
}

// Pairs in order within a hunk: the longest common run of shapes.
function pairs(removed: string[], added: string[]): [number, number][] {
  const n = removed.length, m = added.length;
  const lcs = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] = removed[i] === added[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out: [number, number][] = [];
  for (let i = 0, j = 0; i < n && j < m;) {
    if (removed[i] === added[j]) out.push([i++, j++]);
    else if (lcs[i + 1][j] >= lcs[i][j + 1]) i++;
    else j++;
  }
  return out;
}

const renames = new Map<string, Line[]>();
const note = (from: string, to: string, line: Line) => {
  const key = `${from} → ${to}`;
  renames.set(key, [...(renames.get(key) ?? []), line]);
};

// Within a hunk: a line that came back reshaped by its words alone. The
// pairing is quadratic in the hunk; one past this many line pairs is left to
// the cross-file pass.
const MAX_PAIRS = 4_000_000;
const left: { removed: Line[]; added: Line[] } = { removed: [], added: [] };
for (const h of hunks) {
  const [was, now] = [h.removed, h.added].map((ls) => ls.map((l) => tokens(l.text)));
  const used = [new Set<number>(), new Set<number>()];
  if (h.removed.length && h.added.length && h.removed.length * h.added.length <= MAX_PAIRS) {
    for (const [i, j] of pairs(was.map(shape), now.map(shape))) {
      if (!distinct(was[i])) continue;
      const changed = renamed(was[i], now[j]);
      if (!changed.length) continue;
      for (const [from, to] of new Map(changed)) note(from, to, h.added[j]);
      used[0].add(i);
      used[1].add(j);
    }
  }
  left.removed.push(...h.removed.filter((_, i) => !used[0].has(i)));
  left.added.push(...h.added.filter((_, j) => !used[1].has(j)));
}

// Across files: a line removed in one file and added in another, the same
// or reshaped by its words alone, where its shape is the one of its kind on
// each side, so a common line pairs with nothing.
const once = (ls: Line[]) => {
  const by = new Map<string, Line | null>();
  for (const l of ls) {
    const ts = tokens(l.text);
    if (!distinct(ts)) continue;
    const s = shape(ts);
    by.set(s, by.has(s) ? null : l);
  }
  return by;
};
const [gone, come] = [once(left.removed), once(left.added)];
const moves = new Map<string, number>();
for (const [s, a] of gone) {
  const b = come.get(s);
  if (!a || !b || a.file === b.file) continue;
  const changed = renamed(tokens(a.text), tokens(b.text));
  for (const [from, to] of new Map(changed)) note(from, to, b);
  const key = `${a.file} → ${b.file}`;
  moves.set(key, (moves.get(key) ?? 0) + 1);
}

// A move of a few lines is a common line, not a structure.
const MIN_MOVED = 5;
const moved = [...moves].filter(([, n]) => n >= MIN_MOVED).sort((a, b) => b[1] - a[1]);
const named = [...renames].sort((a, b) => b[1].length - a[1].length);

if (!moved.length && !named.length) {
  console.log("check-kept: nothing removed came back under another name or place");
  Deno.exit(0);
}
console.log("check-kept: kept under another name or place — report each as kept, or say why it is not");
for (const [key, n] of moved) console.log(`  moved    ${key}  ${n} line(s)`);
for (const [key, lines] of named) {
  const where = lines.slice(0, 3).map((l) => `${l.file}:${l.at}`).join(", ");
  console.log(`  renamed  ${key}  ${lines.length} line(s)  ${where}${lines.length > 3 ? ", …" : ""}`);
}
