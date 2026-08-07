// @xpute/core/kit/detect-env.ts

/**
 * [Environment Detector]
 * Detects the runtime environment safely at runtime (avoids bundler interference).
 * Reference: https://github.com/purposeindustries/window-or-global
 */

const g = globalThis as any;

/** [Browser Main Thread] */
function is_browser_main() {
  return typeof g.window !== "undefined" && typeof g.document !== "undefined";
}

/** [Browser Worker] Browser engine worker context (module workers may not expose importScripts). */
function is_browser_worker(): boolean {
  if (typeof g.window !== "undefined") return false;
  if (typeof g.document !== "undefined") return false;
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
