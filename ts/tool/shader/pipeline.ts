// @pxl/shader/pipeline.ts

/**
 * The pipelines the renderer makes over its modules, render and compute,
 * each named by the entry points it runs (pxl-render examples/assembled.rs,
 * pipelines.json). They are made here as WebGPU takes them. A compute
 * pipeline gets the layout its entry declares. A render pipeline gets its
 * vertex buffers; its fragment entry, where it has one, writing one target
 * for each location the entry returns; and a depth buffer, where its record
 * tests depth.
 */

/** A pipeline as pipelines.json describes it. */
export interface Pipeline {
  module: string;
  compute?: string;
  vertex?: string;
  fragment?: string;
  buffers?: GPUVertexBufferLayout[];
  topology?: GPUPrimitiveTopology;
  depth?: boolean;
}

/** The entry points `p` runs, as one line of output names them. */
export function entries(p: Pipeline): string {
  return p.compute ?? [p.vertex, p.fragment].filter(Boolean).join("+");
}

/** A color target for the type a location returns: an integer format for an
 * integer type, and a float format otherwise. */
function target(ty: string): GPUColorTargetState {
  const format = /\bu32\b|vec\du\b/.test(ty)
    ? "r32uint"
    : /\bi32\b|vec\di\b/.test(ty)
      ? "r32sint"
      : "rgba16float";
  return { format };
}

/** The color targets that the fragment entry `fs` of `code` writes: one for
 * each location it returns, alone or as the fields of a struct. */
function targets(code: string, fs: string): GPUColorTargetState[] {
  const ret = code.match(
    new RegExp(`\\bfn\\s+${fs}\\s*\\([^]*?\\)\\s*->\\s*([^{]+)\\{`),
  )?.[1];
  if (!ret) return [];
  if (ret.includes("@location")) return [target(ret)];
  if (ret.includes("@builtin")) return [];
  const body = code.match(
    new RegExp(`struct\\s+${ret.trim()}\\s*\\{([^}]*)\\}`),
  )?.[1];
  return (body ?? "")
    .split(",")
    .filter((f) => f.includes("@location"))
    .map(target);
}

/** Makes `p` over `module`, whose source is `code`, through WebGPU's async
 * call, so that a pipeline WebGPU refuses rejects the promise. */
export function make(
  device: GPUDevice,
  module: GPUShaderModule,
  code: string,
  p: Pipeline,
): Promise<unknown> {
  const label = `${p.module} ${entries(p)}`;
  if (p.compute) {
    return device.createComputePipelineAsync({
      label,
      layout: "auto",
      compute: { module, entryPoint: p.compute },
    });
  }
  return device.createRenderPipelineAsync({
    label,
    layout: "auto",
    vertex: { module, entryPoint: p.vertex, buffers: p.buffers ?? [] },
    fragment: p.fragment
      ? { module, entryPoint: p.fragment, targets: targets(code, p.fragment) }
      : undefined,
    primitive: { topology: p.topology ?? "triangle-list" },
    depthStencil: p.depth
      ? {
          format: "depth32float",
          depthWriteEnabled: true,
          depthCompare: "always",
        }
      : undefined,
  });
}
