#!/usr/bin/env -S deno run -A
// What the deployment has used against its free allowances: R2's writes and
// Hetzner traffic, which the free Workers plan does not bound. Uses
// wrangler's credential and hcloud's active context.
//
//   ./script/usage.ts

import type { f64, u32 } from "@xpute/core/abi/word.ts";

const WORKER = "pixelet";
const BUCKET = "pixelet-tiles";
const SERVER = "pixelet-prod";

const WORKER_REQUESTS_PER_DAY = 100_000;
const R2_STORAGE_BYTES = 10e9;
const R2_CLASS_A_PER_MONTH = 1_000_000;
const R2_CLASS_B_PER_MONTH = 10_000_000;

/** An action in neither list is counted as A, the dearer class, and said aloud. */
const CLASS_A = new Set([
  "ListBuckets",
  "PutBucket",
  "ListObjects",
  "PutObject",
  "CopyObject",
  "CompleteMultipartUpload",
  "CreateMultipartUpload",
  "LifecycleStorageTierTransition",
  "ListMultipartUploads",
  "UploadPart",
  "UploadPartCopy",
  "ListParts",
  "PutBucketEncryption",
  "PutBucketCors",
  "PutBucketLifecycleConfiguration",
]);
const CLASS_B = new Set([
  "HeadBucket",
  "HeadObject",
  "GetObject",
  "UsageSummary",
  "GetBucketEncryption",
  "GetBucketLocation",
  "GetBucketCors",
  "GetBucketLifecycleConfiguration",
]);
const FREE = new Set(["DeleteObject", "DeleteObjects", "DeleteBucket", "AbortMultipartUpload"]);

function die(message: string): never {
  console.error(`[usage] ${message}`);
  Deno.exit(1);
}

async function run(cmd: string, args: string[]): Promise<string> {
  const out = await new Deno.Command(cmd, { args, stdout: "piped", stderr: "piped" }).output();
  if (!out.success) die(`${cmd} ${args.slice(0, 4).join(" ")} failed:\n${new TextDecoder().decode(out.stderr)}`);
  return new TextDecoder().decode(out.stdout);
}

async function token(): Promise<string> {
  const set = Deno.env.get("CLOUDFLARE_API_TOKEN");
  if (set) return set;
  const out = await run("deno", ["run", "-A", "--node-modules-dir=auto", "npm:wrangler", "auth", "token", "--json"]);
  const got = JSON.parse(out.slice(out.indexOf("{"))) as { token?: string };
  return got.token ?? die("wrangler has no credential: run `wrangler login`");
}

async function cloudflare<T>(auth: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`https://api.cloudflare.com/client/v4${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers: { authorization: `Bearer ${auth}`, "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const got = await res.json();
  if (!res.ok || (got.errors && got.errors.length > 0)) die(`${path}: ${JSON.stringify(got.errors ?? got).slice(0, 400)}`);
  return got as T;
}

async function account(auth: string): Promise<string> {
  const set = Deno.env.get("CLOUDFLARE_ACCOUNT_ID");
  if (set) return set;
  const got = await cloudflare<{ result: { id: string }[] }>(auth, "/accounts");
  if (got.result.length !== 1) die(`the credential reaches ${got.result.length} accounts; name one in CLOUDFLARE_ACCOUNT_ID`);
  return got.result[0].id;
}

interface Analytics {
  data: {
    viewer: {
      accounts: {
        workersInvocationsAdaptive: { sum: { requests: u32; errors: u32 }; dimensions: { date: string } }[];
        r2OperationsAdaptiveGroups: { sum: { requests: u32 }; dimensions: { actionType: string } }[];
        r2StorageAdaptiveGroups: { max: { objectCount: u32; payloadSize: f64; metadataSize: f64 } }[];
      }[];
    };
  };
}

const QUERY = `query($account: String!, $worker: String!, $bucket: String!, $monthStart: Date!, $weekStart: Date!, $today: Date!) {
  viewer { accounts(filter: { accountTag: $account }) {
    workersInvocationsAdaptive(limit: 10, filter: { scriptName: $worker, date_geq: $weekStart, date_leq: $today }, orderBy: [date_ASC]) {
      sum { requests errors } dimensions { date }
    }
    r2OperationsAdaptiveGroups(limit: 100, filter: { bucketName: $bucket, date_geq: $monthStart, date_leq: $today }) {
      sum { requests } dimensions { actionType }
    }
    r2StorageAdaptiveGroups(limit: 1, filter: { bucketName: $bucket, date_geq: $monthStart, date_leq: $today }, orderBy: [date_DESC]) {
      max { objectCount payloadSize metadataSize } dimensions { date }
    }
  } }
}`;

const day = (d: Date) => d.toISOString().slice(0, 10);
const count = (n: f64) => Math.round(n).toLocaleString("en-US");
const share = (n: f64, of: f64) => `${((100 * n) / of).toFixed(1)}%`;
const gb = (bytes: f64) => `${(bytes / 1e9).toFixed(2)} GB`;

async function cloudflareUsage(): Promise<void> {
  const auth = await token();
  const accountId = await account(auth);
  const now = new Date();
  const today = day(now);
  const weekStart = day(new Date(now.getTime() - 6 * 86_400_000));
  const monthStart = today.slice(0, 8) + "01";
  const got = await cloudflare<Analytics>(auth, "/graphql", {
    query: QUERY,
    variables: { account: accountId, worker: WORKER, bucket: BUCKET, monthStart, weekStart, today },
  });
  const a = got.data.viewer.accounts[0] ?? die("the analytics returned no account");

  console.log(`Worker '${WORKER}' — requests per UTC day, of ${count(WORKER_REQUESTS_PER_DAY)} free`);
  for (const r of a.workersInvocationsAdaptive) {
    const errors = r.sum.errors > 0 ? `, ${count(r.sum.errors)} errors` : "";
    console.log(`  ${r.dimensions.date}  ${count(r.sum.requests).padStart(9)}  ${share(r.sum.requests, WORKER_REQUESTS_PER_DAY).padStart(6)}${errors}`);
  }

  let classA = 0, classB = 0, free = 0;
  const unknown: string[] = [];
  for (const r of a.r2OperationsAdaptiveGroups) {
    const action = r.dimensions.actionType;
    if (CLASS_B.has(action)) classB += r.sum.requests;
    else if (FREE.has(action)) free += r.sum.requests;
    else {
      classA += r.sum.requests;
      if (!CLASS_A.has(action)) unknown.push(action);
    }
  }
  console.log(`\nR2 '${BUCKET}' — this month since ${monthStart}`);
  console.log(`  Class A  ${count(classA).padStart(11)}  ${share(classA, R2_CLASS_A_PER_MONTH).padStart(6)} of ${count(R2_CLASS_A_PER_MONTH)}`);
  console.log(`  Class B  ${count(classB).padStart(11)}  ${share(classB, R2_CLASS_B_PER_MONTH).padStart(6)} of ${count(R2_CLASS_B_PER_MONTH)}`);
  console.log(`  free     ${count(free).padStart(11)}`);
  if (unknown.length > 0) console.log(`  (counted as Class A, unlisted: ${unknown.join(", ")})`);
  const stored = a.r2StorageAdaptiveGroups[0]?.max;
  if (stored) {
    const bytes = stored.payloadSize + stored.metadataSize;
    console.log(`  stored   ${gb(bytes).padStart(11)}  ${share(bytes, R2_STORAGE_BYTES).padStart(6)} of ${gb(R2_STORAGE_BYTES)}, ${count(stored.objectCount)} objects`);
  }
}

interface Server {
  outgoing_traffic: f64 | null;
  ingoing_traffic: f64 | null;
  included_traffic: f64;
  server_type: { name: string };
}

async function hetznerUsage(): Promise<void> {
  const s = JSON.parse(await run("hcloud", ["server", "describe", SERVER, "-o", "json"])) as Server;
  const out = s.outgoing_traffic ?? 0;
  const gib = (bytes: f64) => `${(bytes / 2 ** 30).toFixed(2)} GiB`;
  console.log(`\nHetzner '${SERVER}' (${s.server_type.name}) — this billing period`);
  console.log(`  outgoing  ${gib(out).padStart(11)}  ${share(out, s.included_traffic).padStart(6)} of ${gib(s.included_traffic)}`);
  console.log(`  incoming  ${gib(s.ingoing_traffic ?? 0).padStart(11)}`);
}

await cloudflareUsage();
await hetznerUsage();
