#!/usr/bin/env -S deno run -A
// What each published build carried, and what it was built from.
//
// The counter says how many builds there have been; this says what they were.
// It exists because nothing else could answer "what went out in b18": the
// number is taken in CI and lives in the bucket, and the commit it named was
// nowhere in the repository.
//
// It records the commit subjects and not a commit id, which is the whole
// point. main's history is rewritten here as a matter of course, so a tag, a
// stored sha and `BUILD_ID`'s own hash all name something that
// may not exist by the time anyone looks. Text written while the history is
// still that history survives every rewrite after it. The sha is kept beside
// it anyway — it resolves right up until it doesn't, and a record that says
// what it was built from is worth more than one that does not.
//
// Beside the counter and for the counter's reason: no squash, rebase or fresh
// clone can touch what is not in git (script/build-counter.ts). R2 has no
// append, so a write is read-add-write, and what keeps two of them from
// losing a line is the workflow's `build-log` concurrency group.
//
// Usage (from the repo root):
//   ./script/build-log.ts                      every build, newest first
//   ./script/build-log.ts --record 18 --web --server     append one (CI)

import type { u32 } from "@xpute/core/abi/word.ts";

const BUCKET = "pixelet-tiles";
const KEY = "_meta/build-log.jsonl";
/** What a build with no number of its own reads to say `#<n>-<id>`
 * (apps/web/vite.config.ts). */
const CACHE = new URL("../apps/web/.build_number", import.meta.url);

/** One published build. `from` is the build this one's commits are counted
 * since, null when there was no previous record to count from. */
interface Entry {
  n: u32;
  sha: string;
  at: string;
  web: boolean;
  server: boolean;
  from: string | null;
  commits: string[];
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

/** The log as it stands. A missing object is refused rather than treated as an
 * empty log: the two are indistinguishable from here, and taking a failed read
 * for an empty one would write a single line over every line there was. */
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
  // Every read is a chance to bring this checkout's cache of the last number
  // it saw published up to date — otherwise only `deno task deploy` writes
  // it, and a checkout that deploys through CI alone shows `#<n>-<id>` against a
  // number that stopped moving. The count itself is the bucket's; this is
  // only the cache, and gitignored.
  if (entries.length > 0) await Deno.writeTextFile(CACHE, `${entries[entries.length - 1].n}\n`);
  return { raw, entries };
}

async function record(n: u32, web: boolean, server: boolean): Promise<void> {
  const sha = git(["rev-parse", "HEAD"]) ?? die("not a git checkout; nothing to record");
  const { raw, entries } = await read();
  // A re-run of a job that wrote its line and then failed for some other
  // reason finds its own record already there. That is the record, so say so
  // and stop rather than appending a second one or reporting a failure.
  if (entries.some((e) => e.n === n)) {
    console.log(`#${n} is already recorded`);
    return;
  }

  // The commits since the last build that was recorded, which is the range a
  // reader means by "what went out in this one". A previous sha that no longer
  // resolves — the history it was on has been rewritten since — leaves this
  // build's own tip as the only honest answer.
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
  console.log(`#${entry.n}  ${entry.at.slice(0, 16).replace("T", " ")}  ${entry.sha.slice(0, 8)}  ${went}`);
  for (const subject of entry.commits) console.log(`    ${subject}`);
}

const args = Deno.args;
if (args[0] === "--record") {
  const n = Number(args[1]);
  if (!Number.isSafeInteger(n) || n < 0) die(`--record wants a build number, not ${JSON.stringify(args[1])}`);
  await record(n, args.includes("--web"), args.includes("--server"));
} else if (args.length === 0) {
  const { entries } = await read();
  for (const entry of entries.slice().reverse()) show(entry);
} else {
  die(`takes no argument but --record: ${JSON.stringify(args[0])}`);
}
