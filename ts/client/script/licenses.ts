// @pxl/client/script/licenses.ts

/**
 * The licenses of what the app is built from and someone else wrote, each
 * with its text, into src/ui/feature/setting/info/licenses.json, which the
 * settings show (setting/info/licenses.tsx).
 *
 *   node script/licenses.ts           writes it (make licenses)
 *   node script/licenses.ts --check   fails where it is not of the lock files now
 *
 * What is listed is what the app holds, not what is installed: the crates the
 * core and the native runtime compile in on the web, iOS and Android
 * (cargo-about over `rust/about.toml`), and the packages each platform's
 * bundle holds, as its source map names them, with those whose native code
 * is linked into the app (Expo's fingerprint, autolinking). A build's tools
 * are in neither. Our own crates and packages are no one else's and are left
 * out. A text several share is kept once. A package that ships no license
 * file takes the file of the package that holds its repository's
 * (licenses.clarify.json), as cargo-about's clarifications do for a crate.
 *
 * The file says the lock files it was made from, and the check takes them
 * again: a dependency changed and the file not made again fails it.
 */

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import fingerprint from "expo/fingerprint";

const CLIENT = fileURLToPath(new URL("..", import.meta.url));
const ROOT = join(CLIENT, "../..");
const OUT = join(CLIENT, "src/ui/feature/setting/info/licenses.json");
const LOCKS = ["rust/Cargo.lock", "ts/pnpm-lock.yaml"];
/** The crates whose dependencies the app compiles in. */
const CRATES = [
  "rust/client/core/Cargo.toml",
  "rust/client/runtime/Cargo.toml",
];
const PLATFORMS = ["web", "ios", "android"];
/** Why Expo's fingerprint holds a package whose native code is linked. */
const LINKED = /Autolinking/;
/** A package's license file by its name. */
const LICENSE_FILE = /^(licen[cs]e|copying)([.-].*)?$/i;

interface Entry {
  name: string;
  version: string;
  license: string;
  text: number;
}

function stamp(): Record<string, string> {
  return Object.fromEntries(
    LOCKS.map((p) => [
      p,
      createHash("sha256")
        .update(readFileSync(join(ROOT, p)))
        .digest("hex")
        .slice(0, 16),
    ]),
  );
}

if (process.argv[2] === "--check") {
  const made = existsSync(OUT)
    ? JSON.parse(readFileSync(OUT, "utf8")).from
    : {};
  const now = stamp();
  if (LOCKS.some((p) => made[p] !== now[p])) {
    console.error(
      `error: ${relative(ROOT, OUT)} is not of the lock files now. Make it again: make licenses`,
    );
    process.exit(1);
  }
  process.exit(0);
}

const texts: string[] = [];
const textIdx = new Map<string, number>();
const entries = new Map<string, Entry>();

function add(name: string, version: string, license: string, text: string) {
  const body = text.replace(/\r\n/g, "\n").trim();
  let i = textIdx.get(body);
  if (i === undefined) {
    i = texts.push(body) - 1;
    textIdx.set(body, i);
  }
  entries.set(`${name}@${version}`, { name, version, license, text: i });
}

// The crates, those from a registry or a repository: a path's is ours.
for (const manifest of CRATES) {
  const about = JSON.parse(
    execFileSync(
      "cargo",
      [
        "about",
        "generate",
        "--format",
        "json",
        "-c",
        join(ROOT, "rust/about.toml"),
        "-m",
        join(ROOT, manifest),
      ],
      {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
        maxBuffer: 1 << 28,
      },
    ),
  ) as {
    licenses: {
      id: string;
      text: string;
      used_by: {
        crate: { name: string; version: string; source: string | null };
      }[];
    }[];
  };
  for (const l of about.licenses) {
    for (const { crate } of l.used_by) {
      if (crate.source) add(crate.name, crate.version, l.id, l.text);
    }
  }
}

// The packages: each platform's bundle, as its source map names its modules,
// and the packages whose native code is linked.
const work = join(ROOT, "tmp/licenses");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
const dirs = new Set<string>();
for (const platform of PLATFORMS) {
  const out = join(work, platform);
  execFileSync(
    "pnpm",
    [
      "exec",
      "expo",
      "export",
      "-p",
      platform,
      "--source-maps",
      "--output-dir",
      out,
    ],
    {
      cwd: CLIENT,
      stdio: "ignore",
      env: { ...process.env, EXPO_NO_TYPESCRIPT_SETUP: "1" },
    },
  );
  for (const map of maps(out)) {
    const m = JSON.parse(readFileSync(map, "utf8")) as {
      sources?: string[];
      sections?: { map: { sources?: string[] } }[];
    };
    const sources = [
      ...(m.sources ?? []),
      ...(m.sections ?? []).flatMap((s) => s.map.sources ?? []),
    ];
    for (const s of sources) {
      const dir = packageDir(join(ROOT, "ts", s));
      if (dir) dirs.add(dir);
    }
  }
}
rmSync(work, { recursive: true, force: true });
const { sources } = await fingerprint.createFingerprintAsync(CLIENT);
for (const s of sources) {
  if (s.type === "dir" || !s.reasons.some((r) => LINKED.test(r))) continue;
  if ("filePath" in s && s.filePath.endsWith("package.json")) {
    dirs.add(dirname(join(CLIENT, s.filePath)));
  }
}

const clarify = JSON.parse(
  readFileSync(join(CLIENT, "script/licenses.clarify.json"), "utf8"),
) as Record<string, string>;
const read = (dir: string) => {
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8")) as {
    name: string;
    version: string;
    license?: string;
  };
  const file = readdirSync(dir).find((f) => LICENSE_FILE.test(f));
  return { pkg, text: file && readFileSync(join(dir, file), "utf8") };
};
const packs = [...dirs].map(read);
const missing: string[] = [];
for (const { pkg, text } of packs) {
  const from = clarify[pkg.name];
  const found =
    text ?? (from && packs.find((r) => r.pkg.name === from && r.text)?.text);
  if (!found) {
    missing.push(`${pkg.name}@${pkg.version}`);
    continue;
  }
  add(pkg.name, pkg.version, pkg.license ?? "", found);
}
if (missing.length > 0) {
  console.error(
    `error: no license file in ${missing.join(", ")}; name the package whose file is theirs in script/licenses.clarify.json`,
  );
  process.exit(1);
}

const packages = [...entries.values()].sort((a, b) =>
  a.name === b.name
    ? a.version.localeCompare(b.version)
    : a.name.localeCompare(b.name),
);
writeFileSync(
  OUT,
  `${JSON.stringify({ $generated: "script/licenses.ts; make licenses", from: stamp(), texts, packages }, null, 1)}\n`,
);
console.log(
  `${packages.length} packages, ${texts.length} texts: ${relative(ROOT, OUT)}`,
);

/** The source maps under `dir`. */
function maps(dir: string): string[] {
  return readdirSync(dir, { recursive: true, encoding: "utf8" })
    .filter((f) => f.endsWith(".map"))
    .map((f) => join(dir, f));
}

/** The installed package a module lies in, none for our own: a module of a
 * workspace package is reached by its own path, not through node_modules. */
function packageDir(module: string): string | undefined {
  const at = module.lastIndexOf("/node_modules/");
  if (at < 0) return undefined;
  const parts = module.slice(at + "/node_modules/".length).split("/");
  const name = parts[0]!.startsWith("@")
    ? parts.slice(0, 2).join("/")
    : parts[0]!;
  if (name === ".pnpm") return undefined;
  return join(module.slice(0, at), "node_modules", name);
}
