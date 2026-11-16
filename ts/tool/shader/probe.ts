// @pxl/shader/probe.ts

/// <reference types="vite/client" />

/**
 * The dev server's shader probe (probe.html). It makes each module as the
 * renderer assembles it, with every pipeline the renderer makes over it, on
 * this device alone, one at a time. `?only=a,b` probes only the modules whose
 * names start with those prefixes. `?bisect=<name>` finds the line a device
 * refuses, and `?from=` with `?to=` narrows the search to a range of lines.
 * `?src=` probes one of the experiments' recorded shaders, named by the end
 * of its path, made into a pipeline over its first compute entry, or else over
 * its first vertex and fragment entries.
 */

import { round } from "@xpute/kit/math/scalar.ts";

import { entries, make, type Pipeline } from "./pipeline.ts";
import { install } from "./remote-console.ts";

const RECORDED = import.meta.glob<string>(
  "../../../doc/**/experiment/**/*.wgsl",
  {
    query: "?raw",
    import: "default",
  },
);

async function assembled(): Promise<{
  modules: Record<string, string>;
  pipelines: Pipeline[];
}> {
  const r = await fetch("/__assembled");
  if (!r.ok) throw new Error(`no assembled modules: ${await r.text()}`);
  return r.json();
}

install();

const out = document.getElementById("out")!;

function show(line: string, ok: boolean): void {
  const div = document.createElement("div");
  div.className = ok ? "ok" : "bad";
  div.textContent = line;
  out.append(div);
}

function entry(code: string, stage: string): string | undefined {
  return code.match(new RegExp(`@${stage}[^]*?\\bfn\\s+(\\w+)\\s*\\(`))?.[1];
}

function first(name: string, code: string): Pipeline[] {
  const cs = entry(code, "compute");
  if (cs) return [{ module: name, compute: cs }];
  const vertex = entry(code, "vertex");
  if (!vertex) return [];
  return [{ module: name, vertex, fragment: entry(code, "fragment") }];
}

async function probe(
  device: GPUDevice,
  name: string,
  code: string,
  pipelines: Pipeline[],
  quiet = false,
): Promise<boolean> {
  const say = (
    what: string,
    errs: (string | null | undefined)[],
    t0: number,
  ) => {
    const ms = round(performance.now() - t0);
    const said = errs.filter(Boolean);
    const line =
      said.length === 0
        ? `[probe] ${name} ${what}: ok (${ms} ms)`
        : `[probe] ${name} ${what}: ${said.join(" | ").slice(0, 600)} (${ms} ms)`;
    if (!quiet) {
      console[said.length === 0 ? "info" : "error"](line);
      show(line, said.length === 0);
    }
    return said.length === 0;
  };
  let t0 = performance.now();
  device.pushErrorScope("validation");
  device.pushErrorScope("internal");
  const module = device.createShaderModule({ label: name, code });
  const errs = (await module.getCompilationInfo()).messages
    .filter((m) => m.type === "error")
    .map((m) => `${m.lineNum}:${m.linePos}: ${m.message}`);
  let internal = await device.popErrorScope();
  let invalid = await device.popErrorScope();
  if (!say("module", [...errs, internal?.message, invalid?.message], t0)) {
    return false;
  }
  let ok = true;
  for (const p of pipelines) {
    t0 = performance.now();
    device.pushErrorScope("validation");
    device.pushErrorScope("internal");
    let said: string | null = null;
    try {
      await make(device, module, code, p);
    } catch (e) {
      const reason = (e as GPUPipelineError).reason ?? "error";
      said = `${reason}: ${(e as Error).message}`;
    }
    internal = await device.popErrorScope();
    invalid = await device.popErrorScope();
    ok = say(entries(p), [said, internal?.message, invalid?.message], t0) && ok;
  }
  return ok;
}

/** Where in the longest function of `code` a `return;` may be put: before a
 * line that starts a statement, right after a line that ends one, and outside
 * any `continuing` block. */
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
  // Within `range` (lines counted from 1), cuts stand only at the depth of the
  // range's first line. A `return;` there drops only the rest of that block,
  // so a later cut always keeps more of the code than an earlier one, which
  // the bisection needs.
  let depth = -1;
  for (let i = best.from + 1; i < best.to; i++) {
    const t = lines[i].trim();
    const prev = lines[i - 1].trim();
    const in_continuing = stack.includes(true);
    if (range && i === range[0] - 1) depth = stack.length;
    const in_range =
      !range ||
      (i >= range[0] - 1 && i <= range[1] - 1 && stack.length === depth);
    if (
      in_range &&
      !in_continuing &&
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

/** Finds the first line of `name`'s longest function past which the device
 * refuses the module. Each probe puts a `return;` before a candidate line,
 * and the candidates are halved between one the device takes and one it
 * refuses. */
async function bisect(
  device: GPUDevice,
  name: string,
  code: string,
  pipelines: Pipeline[],
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
  if (!(await probe(device, name, cut(0), pipelines, true))) {
    return say("refused with nothing past the function's start", false);
  }
  if (await probe(device, name, cut(at.length), pipelines, true)) {
    return say("taken whole");
  }
  // The device takes cut(lo) and refuses cut(hi).
  let [lo, hi] = [0, at.length];
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (await probe(device, name, cut(mid), pipelines, true)) lo = mid;
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
    const name = src.split("/").pop()!;
    await probe(device, name, code, first(name, code));
    console.info("[probe] done");
    return show("done", true);
  }
  const { modules, pipelines } = await assembled();
  const over = (name: string) => pipelines.filter((p) => p.module === name);
  const target = params.get("bisect");
  if (target) {
    const code = modules[target];
    if (code === undefined)
      return void console.error(`[probe] no module ${target}`);
    const [a, b] = [params.get("from"), params.get("to")].map(Number);
    await bisect(
      device,
      target,
      code,
      over(target),
      a && b ? [a, b] : undefined,
    );
    console.info("[probe] done");
    return show("done", true);
  }
  const only = params.get("only")?.split(",").filter(Boolean);
  const names = Object.keys(modules)
    .filter((n) => !only || only.some((o) => n.startsWith(o)))
    .sort((a, b) => a.localeCompare(b));
  out.textContent = `${names.length} modules`;
  for (const name of names) {
    await probe(device, name, modules[name], over(name));
  }
  console.info("[probe] done");
  show("done", true);
}

void main();
