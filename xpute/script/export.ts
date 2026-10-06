#!/usr/bin/env -S deno run -A
// Copies what git tracks under xpute/ into xpute's own repository byte for
// byte, its root being this directory. Prints a commit message naming the
// pixelet commit it came from, for `git commit -F -` there.
//
//   deno task export --to ../xpute [--check]

import { dirname, join, relative } from "@std/path";

const XPUTE = dirname(dirname(new URL(import.meta.url).pathname));

/** Neither written nor compared: build output and lock files. */
const UNMANAGED = ["rust/target/", "target/", "node_modules/", "rust/Cargo.lock", "deno.lock"];

const unmanaged = (path: string) => UNMANAGED.some((u) => (u.endsWith("/") ? path.startsWith(u) : path === u));

function git(args: string[], cwd = XPUTE): string {
  const out = new Deno.Command("git", { args, cwd, stdout: "piped", stderr: "piped" }).outputSync();
  if (!out.success) die(`git ${args.join(" ")}: ${new TextDecoder().decode(out.stderr).trim()}`);
  return new TextDecoder().decode(out.stdout).trimEnd();
}

function die(message: string): never {
  note(message);
  Deno.exit(1);
}

/** stderr, so stdout carries only the commit message. */
function note(message: string): void {
  console.error(`[export] ${message}`);
}

function stamp(): string {
  const dirty = git(["status", "--porcelain", "--", "."]) !== "" ? "-dirty" : "";
  return `pixelet@${git(["rev-parse", "--short", "HEAD"])}${dirty}`;
}

/** The stamp, then the subjects since the one the repository's last commit names, when that still resolves. */
function draft(to: string, at: string): string {
  let previous: string | undefined;
  try {
    previous = git(["log", "-1", "--format=%B"], to).match(/pixelet@([0-9a-f]+)/)?.[1];
  } catch { /* no commit there yet */ }
  const resolves = previous !== undefined &&
    new Deno.Command("git", { args: ["cat-file", "-e", `${previous}^{commit}`], cwd: XPUTE, stdout: "null", stderr: "null" }).outputSync().success;
  const subjects = resolves ? git(["log", "--format=%s", `${previous}..HEAD`, "--", "."]).split("\n").filter((l) => l !== "") : [];
  return subjects.length === 0 ? `${at}\n` : `${at}\n\n${subjects.map((s) => `  ${s}`).join("\n")}\n`;
}

/** `executable` keeps the mode of scripts the tasks call by path. */
type File = { path: string; bytes: Uint8Array; executable: boolean };

function tracked(): File[] {
  // `mode sha stage<TAB>path`, one to a NUL, relative to xpute/.
  const staged = git(["ls-files", "-s", "-z", "--", "."]).split("\0").filter((l) => l !== "");
  if (staged.length === 0) die("git listed nothing to export");
  return staged.map((l) => {
    const path = l.slice(l.indexOf("\t") + 1);
    return { path, bytes: Deno.readFileSync(join(XPUTE, path)), executable: l.startsWith("100755 ") };
  });
}

function same(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((byte, i) => byte === b[i]);
}

function compare(files: File[], to: string): { differs: string[]; extra: string[] } {
  const differs: string[] = [];
  for (const file of files) {
    let held: Uint8Array;
    try {
      held = Deno.readFileSync(join(to, file.path));
    } catch {
      differs.push(file.path);
      continue;
    }
    const runs = ((Deno.statSync(join(to, file.path)).mode ?? 0) & 0o111) !== 0;
    if (!same(file.bytes, held) || runs !== file.executable) differs.push(file.path);
  }
  return { differs, extra: present(to).filter((path) => !files.some((f) => f.path === path)) };
}

/** What the repository holds besides `.git` and what is unmanaged. */
function present(to: string): string[] {
  const out: string[] = [];
  const walk = (dir: string) => {
    for (const entry of Deno.readDirSync(dir)) {
      const full = join(dir, entry.name);
      const path = relative(to, full);
      if (entry.name === ".git" || unmanaged(entry.isDirectory ? `${path}/` : path)) continue;
      if (entry.isDirectory) walk(full);
      else out.push(path);
    }
  };
  try {
    walk(to);
  } catch {
    die(`${to} is not a directory; clone the repository there first`);
  }
  return out;
}

function write(files: File[], to: string): void {
  const kept = new Set(files.map((f) => f.path));
  for (const path of present(to)) if (!kept.has(path)) Deno.removeSync(join(to, path));
  for (const file of files) {
    const full = join(to, file.path);
    Deno.mkdirSync(dirname(full), { recursive: true });
    Deno.writeFileSync(full, file.bytes);
    Deno.chmodSync(full, file.executable ? 0o755 : 0o644);
  }
}

const args = Deno.args;
const toAt = args.indexOf("--to");
if (toAt < 0 || !args[toAt + 1]) die("--to <dir> is required: the xpute repository to export into");
const to = args[toAt + 1];
const files = tracked();

if (args.includes("--check")) {
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
