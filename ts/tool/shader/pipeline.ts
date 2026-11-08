// @pxl/shader/pipeline.ts

/** The pipelines come from the renderer (pxl-render examples/assembled.rs,
 * pipelines.json), each named by the entry points it runs. */

export interface Pipeline {
  module: string;
  compute?: string;
  vertex?: string;
  fragment?: string;
  buffers?: GPUVertexBufferLayout[];
  topology?: GPUPrimitiveTopology;
  depth?: boolean;
}

export function entries(p: Pipeline): string {
  return p.compute ?? [p.vertex, p.fragment].filter(Boolean).join("+");
}

function target(ty: string): GPUColorTargetState {
  const format = /\bu32\b|vec\du\b/.test(ty)
    ? "r32uint"
    : /\bi32\b|vec\di\b/.test(ty)
      ? "r32sint"
      : "rgba16float";
  return { format };
}

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

/** WebGPU's async call is used so that a pipeline WebGPU refuses rejects the
 * promise. */
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
