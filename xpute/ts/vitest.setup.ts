import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

/** A `file:` URL fetched as a page fetches its own files: Golden.load reads
 * the records under spec/ so, and Node's fetch takes no `file:` URL. */
const fetched = globalThis.fetch;
globalThis.fetch = async (input, init) => {
  const url = input instanceof Request ? input.url : String(input);
  if (!url.startsWith("file:")) return fetched(input, init);
  return new Response(await readFile(fileURLToPath(url)));
};
