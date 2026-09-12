// @pxl/api/bake/poi/tools/repack_poi_tiles.ts
//
// Re-encodes a baked POI set from one page format to the current one,
// without the database: a bake's pages are already the provider's
// normalised output, so a format change is a repack of what is on disk.
// Reads `<from>/<page_idx>` (gzipped JSON, the v1/v2 format) and writes
// `<to>/<page_idx>` (gzipped XTP, format.ts's encode_poi_page); `.empty`
// markers are carried across untouched.
//
// Usage (from repo root):
//   deno run -A apps/api/src/bake/poi/tools/repack_poi_tiles.ts --from ./out/poi/v1 [--to ./out/poi/<published>]

import type { Bytes } from "@xpute/core/kit/array.ts";

import type { POI } from "@pxl/shared/bake/model/feature.ts";
import { encode_poi_page } from "@pxl/shared/bake/format.ts";
import { PUBLISHED_VERSION } from "@pxl/shared/bake/published.ts";
import { fmt_bytes, fmt_dur, gzip } from "@pxl/api/bake/bake_util.ts";

const EMPTY_SUFFIX = ".empty";

type Args = { from: string; to: string; concurrency: number };

function parse_args(argv: string[]): Args {
  const args: Args = { from: "", to: `./out/poi/${PUBLISHED_VERSION.poi}`, concurrency: 8 };
  for (let i = 0; i < argv.length; i++) {
    switch (argv[i]) {
      case "--from":
        args.from = argv[++i];
        break;
      case "--to":
        args.to = argv[++i];
        break;
      case "--concurrency":
        args.concurrency = Number(argv[++i]);
        break;
      default:
        throw new Error(`unknown flag: ${argv[i]}`);
    }
  }
  if (!args.from) throw new Error("--from <dir> is required");
  return args;
}

async function gunzip(bytes: Bytes): Promise<Bytes> {
  const body = new Response(bytes).body!.pipeThrough(new DecompressionStream("gzip"));
  return new Uint8Array(await new Response(body).arrayBuffer());
}

async function main(): Promise<void> {
  const args = parse_args(Deno.args);
  console.log(`[repack/poi] from=${args.from} to=${args.to}`);
  await Deno.mkdir(args.to, { recursive: true });

  const names: string[] = [];
  for await (const entry of Deno.readDir(args.from)) if (entry.isFile) names.push(entry.name);
  names.sort();

  let next = 0, pages = 0, empty = 0, poi_total = 0, in_bytes = 0, out_bytes = 0;
  const started = performance.now();
  const decoder = new TextDecoder();

  const worker = async (): Promise<void> => {
    for (;;) {
      const i = next++;
      if (i >= names.length) return;
      const name = names[i];
      if (name.endsWith(EMPTY_SUFFIX)) {
        await Deno.writeTextFile(`${args.to}/${name}`, "");
        empty++;
        continue;
      }
      if (!/^\d+$/.test(name)) continue;
      const gz_in = await Deno.readFile(`${args.from}/${name}`);
      const pois = JSON.parse(decoder.decode(await gunzip(gz_in))) as POI[];
      const gz_out = await gzip(encode_poi_page(pois));
      await Deno.writeFile(`${args.to}/${name}.part`, gz_out);
      await Deno.rename(`${args.to}/${name}.part`, `${args.to}/${name}`);
      pages++;
      poi_total += pois.length;
      in_bytes += gz_in.byteLength;
      out_bytes += gz_out.byteLength;
      if (pages % 2000 === 0) console.log(`[repack/poi] ${pages.toLocaleString()} pages, gz ${fmt_bytes(in_bytes)} -> ${fmt_bytes(out_bytes)}`);
    }
  };
  await Promise.all(Array.from({ length: args.concurrency }, () => worker()));

  const manifest = {
    version: args.to.split("/").filter(Boolean).pop(),
    updated_at: new Date().toISOString(),
    repacked_from: args.from,
    run: { pages, empty, poi_total, gz_bytes: out_bytes },
    content_type: "application/octet-stream",
    content_encoding: "gzip",
    format: "xtp poi page (format.ts encode_poi_page)",
  };
  await Deno.writeTextFile(`${args.to}/manifest.json`, JSON.stringify(manifest, null, 2));
  console.log(
    `[repack/poi] done in ${fmt_dur(performance.now() - started)}: ${pages.toLocaleString()} pages, ${empty.toLocaleString()} empty, ${poi_total.toLocaleString()} POI, gz ${fmt_bytes(in_bytes)} -> ${
      fmt_bytes(out_bytes)
    }`,
  );
}

if (import.meta.main) await main();
