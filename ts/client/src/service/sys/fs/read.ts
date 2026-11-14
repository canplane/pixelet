// @pxl/client/service/sys/fs/read.ts

/** A 204 is a baked page that its version leaves out, and reads as `no-entry`. */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { ComponentError } from "@xpute/link";
import {
  ERROR_CODE_ACCESS,
  ERROR_CODE_IO,
  ERROR_CODE_NO_ENTRY,
  ERROR_CODE_NO_OBJECT,
  ERROR_CODE_REMOTE_UNREACHABLE,
} from "@xpute/link/abi/fs/types.enum.ts";
import { online } from "@/service/origin/online.ts";
import * as origin from "@/service/origin/route.ts";

export async function get(url: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    res = await origin.get(url, signal);
    body = new Uint8Array(await res.arrayBuffer());
  } catch (err) {
    // fetch throws the same TypeError when offline, on a DNS failure, on CORS
    // and on a refused connection. Only the browser's online flag tells
    // offline apart; every other failure is counted as the server's.
    if (!signal.aborted) {
      console.warn(
        `[read] ${url}: ${
          err instanceof Error ? `${err.name}: ${err.message}` : String(err)
        }`,
      );
    }
    throw new ComponentError(
      online() ? ERROR_CODE_IO : ERROR_CODE_REMOTE_UNREACHABLE,
    );
  }
  if (res.status === 204) throw new ComponentError(ERROR_CODE_NO_ENTRY);
  if (!res.ok) {
    console.warn(`[read] ${url}: HTTP ${res.status}`);
    // A 404 is the object's failure: its key names nothing. Any other status
    // is the server's failure.
    throw new ComponentError(
      res.status === 404 ? ERROR_CODE_NO_OBJECT : ERROR_CODE_ACCESS,
    );
  }
  return body;
}
