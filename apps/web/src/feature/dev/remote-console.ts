// @pxl/web/feature/dev/remote-console.ts

/**
 * The console of a device nobody can attach to, forwarded to the terminal the
 * dev server runs in.
 *
 * A phone or a tablet reached over a tunnel has a console, and no way to read
 * it: Safari wants a Mac on a cable, Chrome on Android wants `chrome://inspect`
 * on a desktop, and a WebView may offer neither. So the page that already has
 * a socket to the machine developing it says its lines down that socket
 * instead, and `vite.config.ts` prints them where the server's own output goes.
 *
 * Dev only, and only when this module is imported — `main.tsx` guards it on
 * `import.meta.env.DEV`, which is `false` in every built bundle, so nothing of
 * this reaches a deploy.
 *
 * The page's own console is left doing what it did: this wraps rather than
 * replaces, so a desktop session sees no difference.
 */

const ENDPOINT = "/__console";

/** Lines waiting for the next flush. Batched because a fetch a line would
 * outrun the server on a page that logs in a loop, and because a frame's
 * worth of lines is one useful message rather than twenty. */
const queued: string[] = [];
let scheduled = false;

/** How much of one line is sent. A stack or a packet dump is not what this is
 * for, and a body the server has to hold is a body that can grow without
 * bound on a page that logs a typed array. */
const LINE_MAX = 2000;

/** The name this device goes by in the terminal, so two devices on one server
 * are told apart. The user agent's own words, since nothing else here knows
 * what it is running on. */
function label(): string {
  const ua = navigator.userAgent;
  const which = /iPad|iPhone|iPod/.test(ua) ? "ios" : /Android/.test(ua) ? "android" : /Macintosh/.test(ua) ? "mac" : "web";
  return which;
}

function text(v: unknown): string {
  if (typeof v === "string") return v;
  if (v instanceof Error) return `${v.name}: ${v.message}`;
  try {
    return JSON.stringify(v) ?? String(v);
  } catch {
    return String(v);
  }
}

function flush(): void {
  scheduled = false;
  if (queued.length === 0) return;
  const body = queued.join("\n");
  queued.length = 0;
  // `keepalive` so a line logged as the page goes away still lands, and the
  // failure is swallowed: a console that throws while reporting is worse than
  // one that loses a line.
  fetch(ENDPOINT, { method: "POST", body, keepalive: true }).catch(() => {});
}

function send(level: string, args: unknown[]): void {
  if (queued.length > 200) return;
  queued.push(`[${label()}/${level}] ${args.map(text).join(" ").slice(0, LINE_MAX)}`);
  if (scheduled) return;
  scheduled = true;
  setTimeout(flush, 200);
}

/** Wraps the console and the two events that report what no call to it does:
 * a thrown error nobody caught, and a rejected promise nobody handled. */
export function installRemoteConsole(): void {
  for (const level of ["log", "warn", "error", "info"] as const) {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      original(...args);
      send(level, args);
    };
  }
  addEventListener("error", (e) => send("uncaught", [e.message, `${e.filename}:${e.lineno}`]));
  addEventListener("unhandledrejection", (e) => send("unhandled", [e.reason]));
  console.info(`remote console open — ${navigator.userAgent}`);
}
