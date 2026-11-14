// @pxl/client/remote-console.ts

/** Forwards a page's console to the dev server's terminal, because a phone
 * reached over a tunnel has no devtools to attach. */

const ENDPOINT = "/__console";

const queue: string[] = [];
let scheduled = false;

/** How many of a long message's first and last characters are kept. A
 * compiler's error comes after the source it quotes, so more of the end is
 * kept. */
const HEAD_MAX = 1000;
const TAIL_MAX = 8000;

function clip(s: string): string {
  return s.length <= HEAD_MAX + TAIL_MAX
    ? s
    : `${s.slice(0, HEAD_MAX)}\n… ${s.length - HEAD_MAX - TAIL_MAX} characters …\n${s.slice(-TAIL_MAX)}`;
}

function label(): string {
  const ua = navigator.userAgent;
  const which = /iPad|iPhone|iPod/.test(ua)
    ? "ios"
    : /Android/.test(ua)
      ? "android"
      : /Macintosh/.test(ua)
        ? "mac"
        : "web";
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
  if (queue.length === 0) return;
  const body = queue.join("\n");
  queue.length = 0;
  // `keepalive` delivers a line logged while the page unloads.
  fetch(ENDPOINT, { method: "POST", body, keepalive: true }).then(
    (r) => r.body?.cancel(),
    () => {},
  );
}

function send(level: string, args: unknown[]): void {
  if (queue.length > 200) return;
  queue.push(`[${label()}/${level}] ${clip(args.map(text).join(" "))}`);
  if (scheduled) return;
  scheduled = true;
  setTimeout(flush, 200);
}

export function install(): void {
  for (const level of ["log", "warn", "error", "info"] as const) {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      original(...args);
      send(level, args);
    };
  }
  addEventListener("error", (e) =>
    send("uncaught", [e.message, `${e.filename}:${e.lineno}`]),
  );
  addEventListener("unhandledrejection", (e) => send("unhandled", [e.reason]));
  console.info(`remote console open — ${navigator.userAgent}`);
}
