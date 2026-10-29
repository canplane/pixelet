// .agents/plugins/deno-lsp/proxy.ts

/**
 * `deno lsp` for an agent's LSP tool, which names one workspace folder, the
 * repository. Deno reads a workspace's import map only for the folders it is
 * handed, and the tree has two workspaces (ts/, xpute/ts/), so the initialize
 * request is handed on with those two in its place; every other message
 * passes through as it is.
 *
 * Run as `deno run -A proxy.ts <workspace dir>...`, the directories relative
 * to the repository, which is the initialize request's root.
 */

const folders = Deno.args;
const server = new Deno.Command(Deno.execPath(), {
  args: ["lsp"],
  stdin: "piped",
  stdout: "piped",
  stderr: "inherit",
}).spawn();

const HEADER_END = new TextEncoder().encode("\r\n\r\n");

/** Each message's bytes, header and body, as they arrive on `from`. */
async function* messages(from: ReadableStream<Uint8Array>) {
  let buf = new Uint8Array(0);
  for await (const chunk of from) {
    const next = new Uint8Array(buf.length + chunk.length);
    next.set(buf);
    next.set(chunk, buf.length);
    buf = next;
    for (;;) {
      const end = find(buf, HEADER_END);
      if (end < 0) break;
      const header = new TextDecoder().decode(buf.subarray(0, end));
      const len = Number(/content-length:\s*(\d+)/i.exec(header)?.[1]);
      const total = end + HEADER_END.length + len;
      if (buf.length < total) break;
      yield buf.subarray(end + HEADER_END.length, total);
      buf = buf.subarray(total);
    }
  }
}

function find(hay: Uint8Array, needle: Uint8Array): number {
  outer: for (let i = 0; i + needle.length <= hay.length; i++) {
    for (let j = 0; j < needle.length; j++) {
      if (hay[i + j] !== needle[j]) continue outer;
    }
    return i;
  }
  return -1;
}

function frame(body: Uint8Array): Uint8Array {
  const header = new TextEncoder().encode(
    `Content-Length: ${body.length}\r\n\r\n`,
  );
  const out = new Uint8Array(header.length + body.length);
  out.set(header);
  out.set(body, header.length);
  return out;
}

/** The initialize request with the workspaces in place of its folders. */
function rooted(body: Uint8Array): Uint8Array {
  const msg = JSON.parse(new TextDecoder().decode(body));
  if (msg.method !== "initialize") return body;
  const root: string = msg.params.rootUri ??
    msg.params.workspaceFolders?.[0]?.uri;
  if (root === undefined) return body;
  msg.params.workspaceFolders = folders.map((f) => ({
    uri: `${root.replace(/\/$/, "")}/${f}`,
    name: f,
  }));
  return new TextEncoder().encode(JSON.stringify(msg));
}

const toServer = server.stdin.getWriter();
const toClient = Deno.stdout.writable.getWriter();

const up = (async () => {
  let first = true;
  for await (const body of messages(Deno.stdin.readable)) {
    await toServer.write(frame(first ? rooted(body) : body));
    first = false;
  }
  await toServer.close();
})();

const down = (async () => {
  for await (const body of messages(server.stdout)) {
    await toClient.write(frame(body));
  }
})();

await Promise.race([up, down, server.status]);
Deno.exit((await server.status).code);
