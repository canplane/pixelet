#!/usr/bin/env -S deno run -A
// The binaries that make the repository's data, by what they do and what they
// make: each its crate, its profile, and whether it reads the databases'
// addresses from ./.env. The arguments after the name go to the binary.
//
//   ./script/tool.ts bake <terrain|place|division|earth|star|geoid> [args…]
//   ./script/tool.ts download <pack|moon|earth|star>

interface Tool {
  crate: string;
  bin: string;
  /** `server` where a debug build is felt on every tile; `release` for one long run. */
  profile?: "server" | "release";
  /** Reads db-geo, so runs under ./.env's variables. */
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
};

const [group, name, ...args] = Deno.args;
const tool = TOOLS[group]?.[name];
if (!tool) {
  const usage = Object.entries(TOOLS).map(([g, names]) => `${g} <${Object.keys(names).join("|")}>`).join("\n  ");
  console.error(`usage:\n  ${usage}`);
  Deno.exit(2);
}
const cargo = ["cargo", "run", "--manifest-path", "crates/Cargo.toml", ...(tool.profile ? ["--profile", tool.profile] : []), "-p", tool.crate, "--bin", tool.bin, "--", ...args];
const [cmd, ...rest] = tool.env ? ["./script/with-env.sh", ...cargo] : cargo;
const run = await new Deno.Command(cmd, { args: rest, stdin: "inherit", stdout: "inherit", stderr: "inherit" }).output();
Deno.exit(run.code);
