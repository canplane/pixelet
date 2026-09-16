// @pxl/api/bake/asset/tool/manifest.ts
//
// asset/cube/manifest.json loading/flattening, shared between
// download_texture_pack.ts and manifest.test.ts so both agree on what an
// "entry" is.

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
  /** Free-form material tag (e.g. "generic-painted-plaster-stucco-v1") —
   * descriptive only, not consumed by the download pipeline (which keys
   * its resize algorithm off `style`, not this). */
  material?: string;
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
  /** e.g. "terrain.top.land.bare", "terrain.side.sports", "objects.block.white" — for logging only. */
  label: string;
  url: string;
  file: string;
  /** Normalized (TILE_SIZE) output. */
  dest: URL;
  /** Native-resolution archive, cached and reused as the resize source across runs. */
  originals_dest: URL;
  paletteHex: string | null;
  style: ManifestImageStyle;
};

/** The texture pack's home: the manifest, the normalized images the client
 * draws (`terrain/`, `block/`), and the native-resolution originals. */
export const CUBE_ASSET_DIR = new URL("../../../../../../asset/cube/", import.meta.url);

export async function load_manifest(): Promise<Manifest> {
  const manifest_url = new URL("manifest.json", CUBE_ASSET_DIR);
  return JSON.parse(await Deno.readTextFile(manifest_url));
}

export async function fetch_with_retry(url: string, attempts: number): Promise<Uint8Array> {
  let last_err: unknown;
  for (let i = 1; i <= attempts; i++) {
    try {
      const res = await fetch(url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const buf = new Uint8Array(await res.arrayBuffer());
      if (buf.length === 0) throw new Error("empty response body");
      return buf;
    } catch (e) {
      last_err = e;
      if (i < attempts) await new Promise((r) => setTimeout(r, 400 * i));
    }
  }
  throw last_err instanceof Error ? last_err : new Error(String(last_err));
}

export async function run_pool<T, R>(items: T[], concurrency: number, fn: (item: T) => Promise<R>): Promise<R[]> {
  const results: R[] = new Array(items.length);
  let next = 0;
  async function worker() {
    while (true) {
      const i = next++;
      if (i >= items.length) return;
      results[i] = await fn(items[i]);
    }
  }
  await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, worker));
  return results;
}

export function flatten_manifest(manifest: Manifest): WorkItem[] {
  const textures_dir = CUBE_ASSET_DIR;
  const originals_dir = new URL("original/", CUBE_ASSET_DIR);
  const items: WorkItem[] = [];

  const push = (label: string, entry: ManifestImageEntry) => {
    items.push({
      label,
      url: entry.url,
      file: entry.file,
      dest: new URL(entry.file, textures_dir),
      originals_dest: new URL(entry.file, originals_dir),
      paletteHex: entry.paletteHex ?? null,
      style: entry.style ?? "legacy",
    });
  };

  for (const [ch_key, types] of Object.entries(manifest.terrain.top)) {
    for (const [type_key, entry] of Object.entries(types)) {
      push(`terrain.top.${ch_key}.${type_key}`, entry);
    }
  }
  for (const [type_key, entry] of Object.entries(manifest.terrain.side)) {
    push(`terrain.side.${type_key}`, entry);
  }
  for (const [color_key, entry] of Object.entries(manifest.objects.block)) {
    push(`objects.block.${color_key}`, entry);
  }

  return items;
}
