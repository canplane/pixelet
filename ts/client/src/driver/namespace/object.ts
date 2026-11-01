// @pxl/client/driver/namespace/object.ts

/**
 * OBJECT's backend: the placed objects, a file a shard, which the kernel names
 * by the shard's subject (host.json `root`). For now each is kept in the
 * platform's storage under a key a shard, named as the shard's tile is, its
 * bytes as base64url since storage keeps text. A shard with none has no file:
 * an empty write empties its key, read as no file. A server or a loopback
 * answers the same `open` and `commit` in its place (namespace.ts).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { b64url_to_bytes, bytes_to_b64url } from "@xpute/kit/codec/encoding.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { STORAGE, type Storage } from "@/driver/namespace/storage.ts";
import { tile_name } from "@/driver/namespace/tile.ts";

/** The storage key OBJECT's object `number`, a shard's subject, is kept
 * under; ENOENT for a number that is no tile's. */
export function object_key(number: bigint): string {
  const name = tile_name(number);
  if (name === null) throw new FileError(Errno.ENOENT);
  return `pxl:object:${name}`;
}

/** OBJECT's backend over `s`: a shard's file read whole, ENOENT where none
 * is kept, and written whole. */
export function object_backend(s: Storage = STORAGE): {
  open(number: bigint): U8Array;
  commit(number: bigint, bytes: U8Array): void;
} {
  return {
    open(number) {
      const text = s.get(object_key(number));
      if (text === null || text === "") throw new FileError(Errno.ENOENT);
      try {
        return b64url_to_bytes(text);
      } catch {
        throw new FileError(Errno.EBADMSG);
      }
    },
    commit(number, bytes) {
      s.set(
        object_key(number),
        bytes.byteLength === 0 ? "" : bytes_to_b64url(bytes),
      );
    },
  };
}
