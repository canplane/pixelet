import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

/** Lets a `file:` URL be fetched, as a page fetches its own files.
 * `Golden.load` reads the records under spec/ this way, and Node's own fetch
 * refuses `file:` URLs. */
const node_fetch = globalThis.fetch;
globalThis.fetch = async (input, init) => {
  const url = input instanceof Request ? input.url : String(input);
  if (!url.startsWith("file:")) return node_fetch(input, init);
  return new Response(await readFile(fileURLToPath(url)));
};
