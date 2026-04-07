// @pxl/web/xpute/wasm/bindgen.ts

import type { BindgenModule, BindgenRuntime } from "@xpute/spec/ipc/topology.ts";

const import_module = (url: string): Promise<BindgenModule> => new Function("u", "return import(u)")(url) as Promise<BindgenModule>;

export default {
  load(js_url: string | URL) {
    return import_module(String(js_url));
  },

  init(mod: BindgenModule, wasm_url: string | URL, memory?: WebAssembly.Memory) {
    return mod.default({
      module_or_path: wasm_url,
      ...(memory ? { memory } : {}),
    });
  },

  create_memory(opts) {
    return new WebAssembly.Memory({
      initial: opts?.initial ?? 256,
      maximum: opts?.maximum ?? 4096,
    });
  },
} satisfies BindgenRuntime;
