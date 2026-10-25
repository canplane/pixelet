#!/usr/bin/env -S deno run -A
// The binaries that make the repository's data and the servers a dev run
// starts, by what they do: each its crate, its profile, and whether it reads
// the databases' addresses and the servers' numbers (script/with-env.sh's
// files). The arguments after the name go to the binary.
//
//   ./script/tool.ts bake <terrain|place|division|earth|star|geoid> [args…]
//   ./script/tool.ts download <pack|moon|earth|star>
//   ./script/tool.ts serve <geo|api|tile> [args…]
//   ./script/tool.ts ingest asset      after any change under asset/content/
//   ./script/tool.ts render brand      when ts/client/brand's symbol changes
//
// Built from rust/, where rustup reads the workspace's toolchain, and run
// from the repository's root, where a binary's ./out and its environment's
// files are.

interface Tool {
  crate: string;
  bin: string;
  /** `server` where a debug build is felt on every tile; `release` for one long run. */
  profile?: "server" | "release";
  features?: string;
  /** Given before the caller's own. */
  args?: string[];
  /** Reads db-geo or is a server, so runs under script/with-env.sh's variables. */
  env?: boolean;
}

const TOOLS: Record<string, Record<string, Tool>> = {
  bake: {
    terrain: { crate: "pxl-geo-bake", bin: "bake_terrain", profile: "server", env: true },
    place: { crate: "pxl-geo-bake", bin: "bake_place", profile: "server", env: true },
    division: { crate: "pxl-geo-bake", bin: "bake_division", profile: "server", env: true },
    earth: { crate: "pxl-geo-bake", bin: "bake_earth", profile: "server" },
    star: { crate: "pxl-geo-bake", bin: "bake_star" },
    geoid: { crate: "pxl-tool", bin: "bake_geoid", profile: "release" },
  },
  download: {
    pack: { crate: "pxl-tool", bin: "download_texture_pack" },
    moon: { crate: "pxl-tool", bin: "download_moon" },
    earth: { crate: "pxl-tool", bin: "download_earth" },
    star: { crate: "pxl-tool", bin: "download_star" },
  },
  ingest: {
    asset: { crate: "pxl-tool", bin: "ingest_asset" },
  },
  // The symbol's outline both apps bundle; the page's icons are made by its
  // build (ts/app/web build:brand).
  render: {
    brand: { crate: "pxl-tool", bin: "render_brand", args: ["--symbol"] },
  },
  serve: {
    geo: { crate: "pxl-geo", bin: "pxl-geo", profile: "server", env: true },
    api: { crate: "pxl-api", bin: "pxl-api", profile: "server", env: true },
    // The dev tile server, its sets baked on demand or off the shelf.
    tile: { crate: "pxl-geo-bake", bin: "serve_tiles", profile: "server", features: "serve", env: true },
  },
};

const [group, name, ...args] = Deno.args;
const tool = TOOLS[group]?.[name];
if (!tool) {
  const usage = Object.entries(TOOLS).map(([g, names]) => `${g} <${Object.keys(names).join("|")}>`).join("\n  ");
  console.error(`usage:\n  ${usage}`);
  Deno.exit(2);
}
const root = new URL("../", import.meta.url).pathname;
const flags = [...(tool.profile ? ["--profile", tool.profile] : []), ...(tool.features ? ["--features", tool.features] : []), "-p", tool.crate, "--bin", tool.bin];
// cargo says where it put the binary, whichever target directory it used.
const build = await new Deno.Command("cargo", {
  args: ["build", "--message-format=json-render-diagnostics", ...flags],
  cwd: `${root}rust`,
  stdout: "piped",
  stderr: "inherit",
}).output();
if (!build.success) Deno.exit(build.code || 1);
const exe = new TextDecoder().decode(build.stdout).split("\n").filter((l) => l.startsWith("{")).map((l) => JSON.parse(l))
  .find((m) => m.reason === "compiler-artifact" && m.target?.name === tool.bin && m.executable)?.executable;
if (!exe) {
  console.error(`[tool] cargo built no ${tool.bin}`);
  Deno.exit(1);
}
const given = [...(tool.args ?? []), ...args];
const [cmd, ...rest] = tool.env ? ["./script/with-env.sh", exe, ...given] : [exe, ...given];
const run = await new Deno.Command(cmd, { args: rest, cwd: root, stdin: "inherit", stdout: "inherit", stderr: "inherit" }).output();
Deno.exit(run.code);
