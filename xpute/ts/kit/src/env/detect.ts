// @xpute/kit/env/detect.ts

// Probes globals at runtime rather than at bundle time.

const g = globalThis as unknown as {
  window?: unknown;
  document?: unknown;
  Deno?: unknown;
  postMessage?: unknown;
  addEventListener?: unknown;
  process?: { versions?: { node?: string } };
};

function is_browser_main() {
  return typeof g.window !== "undefined" && typeof g.document !== "undefined";
}

/** A Deno worker also lacks window and document and has postMessage, so Deno is ruled out explicitly. */
function is_browser_worker(): boolean {
  if (typeof g.window !== "undefined") return false;
  if (typeof g.document !== "undefined") return false;
  if (typeof g.Deno !== "undefined") return false;
  if (g.process?.versions?.node) return false;
  return typeof g.postMessage === "function" && typeof g.addEventListener === "function";
}

export const env = {
  is_deno(): boolean {
    return typeof g.Deno !== "undefined";
  },

  is_node(): boolean {
    const p = g.process;
    return !!p?.versions?.node;
  },

  is_browser(): boolean {
    return is_browser_main() || is_browser_worker();
  },

  current(): "DENO" | "NODE" | "BROWSER" | undefined {
    if (this.is_deno()) return "DENO";
    if (this.is_node()) return "NODE";
    if (this.is_browser()) return "BROWSER";
    return undefined;
  },
} as const;
