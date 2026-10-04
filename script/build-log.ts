#!/usr/bin/env -S deno run -A
// What each published build carried. Records commit subjects, since main's
// history is rewritten and a sha may stop resolving. Kept in R2 beside the
// counter; R2 has no append, so the workflow's `build-log` concurrency group
// keeps two writes from losing a line.
//
//   ./script/build-log.ts                      every build, newest first
//   ./script/build-log.ts --record 18 --web --server --smoke success     append one (CI)

import type { u32 } from "@xpute/core/abi/word.ts";

const BUCKET = "pixelet-tiles";
const KEY = "_meta/build-log.jsonl";
const CACHE = new URL("../apps/web/.build_number", import.meta.url);

type Smoke = "pass" | "fail";

interface Entry {
  n: u32;
  sha: string;
  at: string;
  web: boolean;
  server: boolean;
  /** Absent on old lines; `null` where the smoke did not run. */
  smoke?: Smoke | null;
  from: string | null;
  commits: string[];
}

function smokeOf(result: string | undefined): Smoke | null {
  if (result === "success") return "pass";
  if (result === "failure") return "fail";
  return null;
}

const wrangler = (...args: string[]) => ["run", "-A", "--node-modules-dir=auto", "npm:wrangler", ...args];

async function run(args: string[]): Promise<{ ok: boolean; stdout: string; stderr: string }> {
  const out = await new Deno.Command("deno", { args, stdout: "piped", stderr: "piped" }).output();
  return { ok: out.success, stdout: new TextDecoder().decode(out.stdout), stderr: new TextDecoder().decode(out.stderr) };
}

function git(args: string[]): string | null {
  const out = new Deno.Command("git", { args, stdout: "piped", stderr: "null" }).outputSync();
  return out.success ? new TextDecoder().decode(out.stdout).trim() : null;
}

function die(message: string): never {
  console.error(`[build-log] ${message}`);
  Deno.exit(1);
}

/** A missing object is refused: treating it as empty would overwrite the log. */
async function read(): Promise<{ raw: string; entries: Entry[] }> {
  const got = await run(wrangler("r2", "object", "get", `${BUCKET}/${KEY}`, "--remote", "--pipe"));
  if (!got.ok) {
    console.error(got.stderr);
    die(
      `no log at ${KEY}, and an empty one is not assumed. Start it with:\n` +
        `  printf '' | deno run -A --node-modules-dir=auto npm:wrangler r2 object put ${BUCKET}/${KEY} --remote --pipe --content-type application/x-ndjson`,
    );
  }
  const raw = got.stdout;
  const entries: Entry[] = [];
  for (const line of raw.split("\n")) {
    if (line.trim() === "") continue;
    try {
      entries.push(JSON.parse(line) as Entry);
    } catch {
      die(`${KEY} holds a line that is not a record: ${JSON.stringify(line.slice(0, 60))}`);
    }
  }
  // Refreshes the gitignored cache of the last published number.
  if (entries.length > 0) await Deno.writeTextFile(CACHE, `${entries[entries.length - 1].n}\n`);
  return { raw, entries };
}

async function record(n: u32, web: boolean, server: boolean, smoke: Smoke | null): Promise<void> {
  const sha = git(["rev-parse", "HEAD"]) ?? die("not a git checkout; nothing to record");
  const { raw, entries } = await read();
  // A re-run finds its own record already there.
  if (entries.some((e) => e.n === n)) {
    console.log(`#${n} is already recorded`);
    return;
  }

  // A previous sha that no longer resolves leaves only this build's tip.
  const previous = entries.length > 0 ? entries[entries.length - 1].sha : null;
  const reachable = previous !== null && git(["cat-file", "-e", `${previous}^{commit}`]) !== null;
  const range = reachable ? `${previous}..HEAD` : "-1";
  const subjects = git(["log", "--format=%s", range]) ?? "";

  const entry: Entry = {
    n,
    sha,
    at: new Date().toISOString(),
    web,
    server,
    smoke,
    from: reachable ? previous : null,
    commits: subjects.split("\n").filter((s) => s !== ""),
  };

  const file = await Deno.makeTempFile();
  await Deno.writeTextFile(file, `${raw.endsWith("\n") || raw === "" ? raw : raw + "\n"}${JSON.stringify(entry)}\n`);
  const put = await run(wrangler("r2", "object", "put", `${BUCKET}/${KEY}`, "--remote", "--file", file, "--content-type", "application/x-ndjson"));
  await Deno.remove(file);
  if (!put.ok) {
    console.error(put.stderr);
    die(`could not write the log back; #${n} went out and is not recorded`);
  }
  console.log(`#${n} recorded: ${entry.commits.length} commit(s) from ${sha.slice(0, 8)}`);
}

function show(entry: Entry): void {
  const went = [entry.web ? "web" : null, entry.server ? "server" : null].filter(Boolean).join(" + ");
  const smoke = entry.smoke === undefined ? "" : entry.smoke === null ? "  · smoke did not run" : `  · smoke ${entry.smoke}`;
  console.log(`#${entry.n}  ${entry.at.slice(0, 16).replace("T", " ")}  ${entry.sha.slice(0, 8)}  ${went}${smoke}`);
  for (const subject of entry.commits) console.log(`    ${subject}`);
}

const args = Deno.args;
if (args[0] === "--record") {
  const n = Number(args[1]);
  if (!Number.isSafeInteger(n) || n < 0) die(`--record wants a build number, not ${JSON.stringify(args[1])}`);
  const at = args.indexOf("--smoke");
  await record(n, args.includes("--web"), args.includes("--server"), smokeOf(at < 0 ? undefined : args[at + 1]));
} else if (args.length === 0) {
  const { entries } = await read();
  for (const entry of entries.slice().reverse()) show(entry);
} else {
  die(`takes no argument but --record: ${JSON.stringify(args[0])}`);
}
