// @xpute/core/env/detect.ts

/**
 * [Environment Detector]
 * Detects the runtime environment safely at runtime (avoids bundler interference).
 * Reference: https://github.com/purposeindustries/window-or-global
 */

/**
 * Every probe below asks whether a global merely *exists*, so the narrowest
 * honest shape is "these keys may be present, of unknown type" — which types
 * the accesses without claiming anything about what the runtimes actually
 * put there. `process.versions.node` is the one exception: its value is read,
 * not just its presence.
 */
const g = globalThis as unknown as {
  window?: unknown;
  document?: unknown;
  Deno?: unknown;
  postMessage?: unknown;
  addEventListener?: unknown;
  process?: { versions?: { node?: string } };
};

/** [Browser Main Thread] */
function is_browser_main() {
  return typeof g.window !== "undefined" && typeof g.document !== "undefined";
}

/**
 * [Browser Worker] Browser engine worker context (module workers may not expose importScripts).
 *
 * The absence of window/document plus the presence of postMessage and
 * addEventListener is NOT sufficient on its own: a Deno worker matches all
 * four conditions (verified by running one — it reports no window, no
 * document, and both APIs present as bare globals), so the runtime has to be
 * excluded explicitly. Without that, `is_browser()` returns true inside a
 * Deno worker, which is exactly what this function is supposed to rule out.
 */
function is_browser_worker(): boolean {
  if (typeof g.window !== "undefined") return false;
  if (typeof g.document !== "undefined") return false;
  if (typeof g.Deno !== "undefined") return false;
  if (g.process?.versions?.node) return false;
  return typeof g.postMessage === "function" && typeof g.addEventListener === "function";
}

export const env = {
  /** [Server Side] Deno Runtime */
  is_deno(): boolean {
    return typeof g.Deno !== "undefined";
  },

  /** [Server Side] Node.js Runtime */
  is_node(): boolean {
    const p = g.process;
    return !!p?.versions?.node;
  },

  /** [Browser Engine] Browser main thread or browser worker (not Node/Deno workers) */
  is_browser(): boolean {
    return is_browser_main() || is_browser_worker();
  },

  /** [Debug Helper] Returns the current runtime tag */
  current(): "DENO" | "NODE" | "BROWSER" | undefined {
    if (this.is_deno()) return "DENO";
    if (this.is_node()) return "NODE";
    if (this.is_browser()) return "BROWSER";
    return undefined;
  },
} as const;
