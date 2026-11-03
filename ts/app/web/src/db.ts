// @pxl/web/db.ts

/** IndexedDB, one store of files by key; a refused database (sandboxed frame, private window) keeps nothing, never throws. */

import type { Tx } from "@pxl/client/bracket.tsx";

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
        // The browser lost the connection, as Safari does in the background.
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

function asked<R>(req: IDBRequest<R>): Promise<R> {
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
  // `fn`'s own rejection is the one thrown, and its abort this one's.
  committed.catch(() => {});
  try {
    const out = await fn({
      get: (key) =>
        asked<Uint8Array<ArrayBuffer> | undefined>(store.get(key)).then(
          (bytes) => bytes ?? null,
        ),
      put: async (key, bytes) => {
        await (bytes === null
          ? asked(store.delete(key))
          : asked(store.put(bytes, key)));
      },
    });
    await committed;
    return out;
  } catch (err) {
    try {
      t.abort();
    } catch {
      // Already over.
    }
    throw err;
  }
}
