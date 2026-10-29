// @pxl/web/remote-console.ts

/**
 * Forwards a device's console to the dev server's terminal, since a phone over
 * a tunnel has no attachable devtools. Dev only: main.tsx imports it under
 * `import.meta.env.DEV`. Wraps the console rather than replacing it.
 */

const ENDPOINT = "/__console";

const queued: string[] = [];
let scheduled = false;

const LINE_MAX = 2000;

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
  if (queued.length === 0) return;
  const body = queued.join("\n");
  queued.length = 0;
  // `keepalive` lands a line logged as the page unloads; a failure is swallowed.
  fetch(ENDPOINT, { method: "POST", body, keepalive: true }).then(
    (r) => r.body?.cancel(),
    () => {},
  );
}

function send(level: string, args: unknown[]): void {
  if (queued.length > 200) return;
  queued.push(
    `[${label()}/${level}] ${args.map(text).join(" ").slice(0, LINE_MAX)}`,
  );
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
