// @pxl/web/kernel/io/load.ts

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { FileError } from "@xpute/runtime/ipc/sys.ts";
import { loadKey } from "@/catalog/asset/load.ts";
import { originGet } from "@platform/origin.ts";
import { assetUrl } from "@/config/route.ts";

export async function read_asset(load: u32, signal: AbortSignal): Promise<U8Array> {
  const key = loadKey(load);
  if (key === null) throw new FileError(Errno.ENOENT);
  const res = await originGet(assetUrl(key), signal);
  if (!res.ok) throw new FileError(res.status === 404 ? Errno.ENOENT : Errno.EACCES, res.status);
  return new Uint8Array(await res.arrayBuffer());
}
