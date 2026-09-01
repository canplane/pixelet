// @pxl/shared/assets/cell/tools/manifest.ts
//
// pixelet_ms2_texture_manifest.json (repo root) loading/flattening, shared
// between download_texture_pack.ts (fetches+normalizes every entry) and
// manifest.test.ts (validates the manifest's own shape/distinctness) — kept
// here rather than duplicated so both always agree on what an "entry" is.

import { parse as parse_jsonc } from "jsr:@std/jsonc@^1";

/** "cute-painted-fantasy-v1" sources are hand-painted at a size well above
 * 64x64 and need a real smoothing resample (see download_texture_pack.ts's
 * lanczos_resize) or they'd re-pixelate on the way down; "legacy" sources
 * are the original pixel-art set and keep nearest-neighbor. Absent (an
 * older manifest, or a hand-edited entry) defaults to "legacy" — the
 * conservative choice, since nearest-neighbor never blurs a source that
 * genuinely was already pixel-perfect at 64x64. */
export type ManifestImageStyle = "cute-painted-fantasy-v1" | "legacy";

export type ManifestImageEntry = {
  sourceCreation?: number;
  sourceCreationId?: string;
  url: string;
  file: string;
  paletteHex?: string | null;
  style?: ManifestImageStyle;
};

export type Manifest = {
  terrain: {
    top: Record<string, Record<string, ManifestImageEntry>>;
    side: Record<string, ManifestImageEntry>;
  };
  objects: {
    block: Record<string, ManifestImageEntry>;
  };
};

export type WorkItem = {
  /** e.g. "terrain.top.land.BARE", "terrain.side.SPORTS", "objects.block.white" — for logging only. */
  label: string;
  url: string;
  file: string;
  /** Destination file, resolved against ../textures/. */
  dest: URL;
  paletteHex: string | null;
  style: ManifestImageStyle;
};

/** Walks up looking for the WORKSPACE ROOT deno.json specifically — several
 * workspace members (e.g. packages/pxl/shared) carry their own smaller
 * deno.json too, so a plain "does deno.json exist" check would stop one
 * level too early; only the root config declares "workspace". */
export async function find_repo_root(): Promise<URL> {
  let dir = new URL(".", import.meta.url);
  for (let i = 0; i < 12; i++) {
    try {
      // deno.json is JSONC (allows comments) — the repo root's own config
      // has several, so a strict JSON.parse fails on it specifically.
      const config = parse_jsonc(await Deno.readTextFile(new URL("deno.json", dir))) as { workspace?: unknown };
      if (Array.isArray(config.workspace)) return dir;
    } catch {
      // no deno.json here at all — keep walking up
    }
    const parent = new URL("../", dir);
    if (parent.href === dir.href) break;
    dir = parent;
  }
  throw new Error("could not locate workspace-root deno.json walking up from " + import.meta.url);
}

export async function load_manifest(): Promise<Manifest> {
  const repo_root = await find_repo_root();
  const manifest_url = new URL("pixelet_ms2_texture_manifest.json", repo_root);
  return JSON.parse(await Deno.readTextFile(manifest_url));
}

export function flatten_manifest(manifest: Manifest): WorkItem[] {
  const textures_dir = new URL("../textures/", import.meta.url);
  const items: WorkItem[] = [];

  const push_terrain = (label: string, entry: ManifestImageEntry) => {
    const filename = entry.file.replace(/^terrain\//, "");
    items.push({
      label,
      url: entry.url,
      file: entry.file,
      dest: new URL(`terrain/${filename}`, textures_dir),
      paletteHex: entry.paletteHex ?? null,
      style: entry.style ?? "legacy",
    });
  };

  for (const [ch_key, types] of Object.entries(manifest.terrain.top)) {
    for (const [type_key, entry] of Object.entries(types)) {
      push_terrain(`terrain.top.${ch_key}.${type_key}`, entry);
    }
  }
  for (const [type_key, entry] of Object.entries(manifest.terrain.side)) {
    push_terrain(`terrain.side.${type_key}`, entry);
  }
  for (const [color_key, entry] of Object.entries(manifest.objects.block)) {
    const filename = entry.file.replace(/^objects\//, "");
    items.push({
      label: `objects.block.${color_key}`,
      url: entry.url,
      file: entry.file,
      dest: new URL(`objects/${filename}`, textures_dir),
      paletteHex: entry.paletteHex ?? null,
      style: entry.style ?? "legacy",
    });
  }

  return items;
}
