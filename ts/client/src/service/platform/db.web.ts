// @pxl/client/service/platform/db.web.ts

/** Where the browser refuses a database, as in a sandboxed frame or a private
 * window, this keeps nothing and never throws. */

import type { Tx } from "./types.ts";

const STORE = "file";

const NOTHING: Tx = {
  get: () => Promise.resolve(null),
  put: () => Promise.resolve(),
};

function open(): Promise<IDBDatabase | null> {
  return new Promise((resolve) => {
    try {
      const req = globalThis.indexedDB.open("pxl", 1);
      req.onupgradeneeded = () => req.result.createObjectStore(STORE);
      req.onsuccess = () => {
        // The browser closed the connection, as Safari does in the
        // background, so the database is opened again.
        req.result.onclose = () => (db = open());
        resolve(req.result);
      };
      req.onerror = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
}

let db = open();

function settle<R>(req: IDBRequest<R>): Promise<R> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function transaction<T>(fn: (tx: Tx) => Promise<T>): Promise<T> {
  const idb = await db;
  if (idb === null) return fn(NOTHING);
  const t = idb.transaction(STORE, "readwrite");
  const store = t.objectStore(STORE);
  const committed = new Promise<void>((resolve, reject) => {
    t.oncomplete = () => resolve();
    t.onabort = () => reject(t.error);
  });
  // When `fn` rejects, its own rejection is the one thrown. The abort that
  // follows rejects `committed` too, and this catch keeps that second
  // rejection from going unhandled.
  committed.catch(() => {});
  try {
    const out = await fn({
      get: (key) =>
        settle<Uint8Array<ArrayBuffer> | undefined>(store.get(key)).then(
          (bytes) => bytes ?? null,
        ),
      put: async (key, bytes) => {
        await (bytes === null
          ? settle(store.delete(key))
          : settle(store.put(bytes, key)));
      },
    });
    await committed;
    return out;
  } catch (err) {
    try {
      t.abort();
    } catch {
      // The transaction has already finished, so there is nothing to abort.
    }
    throw err;
  }
}
