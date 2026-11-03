// @pxl/web/probe.ts

/**
 * The dev server's shader probe (probe.html): each module beside the
 * renderer's shaders made into a pipeline on this device alone, one at a
 * time, and what the device said of it, on the page and to the dev server's
 * console (remote-console.ts). A compute module's entry is made with its own
 * layout; a module with a vertex and a fragment entry is made into a render
 * pipeline whose targets are as many rgba16float ones as its fragment
 * returns. `?only=a,b` probes the modules whose names start so.
 */

import { round } from "@xpute/kit/math/scalar.ts";

import { install } from "./remote-console.ts";

/** The experiments' shaders, which `?src=` names by its path under doc/experiment. */
const RECORDED = import.meta.glob<string>(
  "../../../../doc/experiment/**/*.wgsl",
  { query: "?raw", import: "default" },
);

const SOURCES = import.meta.glob<string>(
  "../../../../rust/client/render/src/frame/shader/*.wgsl",
  { query: "?raw", import: "default" },
);

const out = document.getElementById("out")!;

function show(line: string, ok: boolean): void {
  const div = document.createElement("div");
  div.className = ok ? "ok" : "bad";
  div.textContent = line;
  out.append(div);
}

/** The first entry of `stage` in `code`, by its attribute. */
function entry(code: string, stage: string): string | undefined {
  return code.match(new RegExp(`@${stage}[^]*?\\bfn\\s+(\\w+)\\s*\\(`))?.[1];
}

/** How many locations the fragment entry `fs` writes: one, or its struct's. */
function targets(code: string, fs: string): number {
  const ret = code.match(
    new RegExp(`\\bfn\\s+${fs}\\s*\\([^]*?\\)\\s*->\\s*([^{]+)\\{`),
  )?.[1];
  if (!ret) return 0;
  if (ret.includes("@location")) return 1;
  const name = ret.trim();
  const body = code.match(
    new RegExp(`struct\\s+${name}\\s*\\{([^}]*)\\}`),
  )?.[1];
  return body ? (body.match(/@location/g) ?? []).length : 1;
}

async function probe(
  device: GPUDevice,
  name: string,
  code: string,
  quiet = false,
): Promise<boolean> {
  const t0 = performance.now();
  device.pushErrorScope("validation");
  device.pushErrorScope("internal");
  const module = device.createShaderModule({ label: name, code });
  const cs = entry(code, "compute");
  const [vs, fs] = [entry(code, "vertex"), entry(code, "fragment")];
  let kind: string;
  let made: Promise<unknown>;
  if (cs) {
    kind = cs;
    made = device.createComputePipelineAsync({
      label: name,
      layout: "auto",
      compute: { module, entryPoint: cs },
    });
  } else if (vs) {
    kind = fs ? `${vs}+${fs}` : vs;
    const n = fs ? targets(code, fs) : 0;
    made = device.createRenderPipelineAsync({
      label: name,
      layout: "auto",
      vertex: { module, entryPoint: vs },
      fragment: fs
        ? {
            module,
            entryPoint: fs,
            targets: Array.from({ length: n }, () => ({
              format: "rgba16float" as GPUTextureFormat,
            })),
          }
        : undefined,
      primitive: { topology: "triangle-list" },
    });
  } else {
    kind = "no entry";
    made = Promise.resolve();
  }
  let said: string | null = null;
  try {
    await made;
  } catch (e) {
    const reason = (e as GPUPipelineError).reason ?? "error";
    said = `${reason}: ${(e as Error).message}`;
  }
  const internal = await device.popErrorScope();
  const invalid = await device.popErrorScope();
  const ms = round(performance.now() - t0);
  const errs = [said, internal?.message, invalid?.message].filter(Boolean);
  const line =
    errs.length === 0
      ? `[probe] ${name} ${kind}: ok (${ms} ms)`
      : `[probe] ${name} ${kind}: ${errs.join(" | ").slice(0, 600)} (${ms} ms)`;
  if (!quiet) {
    console[errs.length === 0 ? "info" : "error"](line);
    show(line, errs.length === 0);
  }
  return errs.length === 0;
}

/** Where in the longest function of `code` a `return;` may stand: before a
 * line that starts a statement, outside any `continuing` block, the line
 * before it ending one. */
function cuts(
  lines: string[],
  range?: [number, number],
): { from: number; at: number[] } {
  let best = { from: 0, to: 0 };
  let open = -1;
  lines.forEach((l, i) => {
    if (/^fn \w+\(/.test(l)) open = i;
    else if (l === "}" && open >= 0) {
      if (i - open > best.to - best.from) best = { from: open, to: i };
      open = -1;
    }
  });
  const at: number[] = [];
  const stack: boolean[] = [];
  // Within `range` (lines from 1), only at the depth its first line stands
  // at: a `return;` there drops the rest of that block alone, so the cuts
  // drop more the later they stand.
  let depth = -1;
  for (let i = best.from + 1; i < best.to; i++) {
    const t = lines[i].trim();
    const prev = lines[i - 1].trim();
    const inContinuing = stack.includes(true);
    if (range && i === range[0] - 1) depth = stack.length;
    const inRange =
      !range ||
      (i >= range[0] - 1 && i <= range[1] - 1 && stack.length === depth);
    if (
      inRange &&
      !inContinuing &&
      !t.startsWith("}") &&
      !t.startsWith("var ") &&
      /[;{}]$/.test(prev)
    ) {
      at.push(i);
    }
    for (const ch of t) {
      if (ch === "{") stack.push(t.startsWith("continuing"));
      else if (ch === "}") stack.pop();
    }
  }
  return { from: best.from, at };
}

/** The first line of `name`'s longest function past which the device
 * refuses it: each probe the module with a `return;` before a candidate
 * line, halving the candidates between one it takes and one it refuses. */
async function bisect(
  device: GPUDevice,
  name: string,
  code: string,
  range?: [number, number],
): Promise<void> {
  const lines = code.split("\n");
  const { at } = cuts(lines, range);
  show(`${at.length} cuts at lines ${at.map((i) => i + 1).join(" ")}`, true);
  const cut = (k: number) =>
    k >= at.length
      ? code
      : [...lines.slice(0, at[k]), "return;", ...lines.slice(at[k])].join("\n");
  const say = (s: string, ok = true) => {
    console[ok ? "info" : "error"](`[probe] bisect ${name}: ${s}`);
    show(s, ok);
  };
  if (!(await probe(device, name, cut(0), true))) {
    return say("refused with nothing past the function's start", false);
  }
  if (await probe(device, name, cut(at.length), true)) {
    return say("taken whole");
  }
  // cut(lo) taken, cut(hi) refused.
  let [lo, hi] = [0, at.length];
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (await probe(device, name, cut(mid), true)) lo = mid;
    else hi = mid;
    show(`${lo}..${hi} of ${at.length}`, true);
  }
  const from = at[lo];
  const to = hi < at.length ? at[hi] : lines.length;
  say(
    `refused once lines ${from + 1}..${to} are in:\n${lines
      .slice(Math.max(0, from - 3), to + 1)
      .map((l, i) => `${Math.max(0, from - 3) + i + 1}: ${l}`)
      .join("\n")}`,
    false,
  );
}

async function main(): Promise<void> {
  const adapter = await navigator.gpu?.requestAdapter({
    powerPreference: "high-performance",
  });
  if (!adapter) {
    console.error("[probe] no adapter");
    return;
  }
  const { vendor, architecture, device: dev, description } = adapter.info;
  console.info(
    `[probe] adapter: ${[vendor, architecture, dev, description].filter(Boolean).join(" · ")}`,
  );
  const device = await adapter.requestDevice({
    requiredFeatures: adapter.features.has("depth32float-stencil8")
      ? ["depth32float-stencil8"]
      : [],
  });
  device.lost.then((i) => console.warn(`[probe] device lost: ${i.message}`));
  const params = new URLSearchParams(location.search);
  const src = params.get("src");
  if (src) {
    const path = Object.keys(RECORDED).find((p) => p.endsWith(`/${src}`));
    if (!path) return void console.error(`[probe] no recorded shader ${src}`);
    const code = await RECORDED[path]();
    await probe(device, src.split("/").pop()!, code);
    console.info("[probe] done");
    return show("done", true);
  }
  const target = params.get("bisect");
  if (target) {
    const path = Object.keys(SOURCES).find((p) =>
      p.endsWith(`/${target}.wgsl`),
    );
    if (!path) return void console.error(`[probe] no module ${target}`);
    const [a, b] = [params.get("from"), params.get("to")].map(Number);
    await bisect(
      device,
      target,
      await SOURCES[path](),
      a && b ? [a, b] : undefined,
    );
    console.info("[probe] done");
    return show("done", true);
  }
  const only = params.get("only")?.split(",").filter(Boolean);
  const names = Object.keys(SOURCES)
    .map(
      (p) =>
        [
          p,
          p
            .split("/")
            .pop()!
            .replace(/\.wgsl$/, ""),
        ] as const,
    )
    .filter(([, n]) => !only || only.some((o) => n.startsWith(o)))
    .sort(([, a], [, b]) => a.localeCompare(b));
  out.textContent = `${names.length} modules`;
  for (const [path, name] of names) {
    await probe(device, name, await SOURCES[path]());
  }
  console.info("[probe] done");
  show("done", true);
}

void main();
