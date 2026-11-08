// @pxl/client/service/platform/db.native.ts

/** A device's database: SQLite, with one table of files by key, in the app's
 * own files. */

import type { Tx } from "./types.ts";
import { openDatabaseAsync } from "expo-sqlite";

const db = openDatabaseAsync("pxl.db").then(async (d) => {
  await d.execAsync(
    "CREATE TABLE IF NOT EXISTS file (key TEXT PRIMARY KEY, bytes BLOB NOT NULL) WITHOUT ROWID",
  );
  return d;
});

/** The last transaction asked for, settled. Transactions run one after
 * another, because a SQLite transaction takes in every query made while it
 * is open, even one meant for another transaction. */
let last: Promise<unknown> = db.catch(() => {});

export function transaction<T>(fn: (tx: Tx) => Promise<T>): Promise<T> {
  const run = last.then(async () => {
    const d = await db;
    const tx: Tx = {
      get: async (key) => {
        const row = await d.getFirstAsync<{ bytes: Uint8Array<ArrayBuffer> }>(
          "SELECT bytes FROM file WHERE key = ?",
          key,
        );
        return row?.bytes ?? null;
      },
      put: async (key, bytes) => {
        await (bytes === null
          ? d.runAsync("DELETE FROM file WHERE key = ?", key)
          : d.runAsync(
              "INSERT OR REPLACE INTO file (key, bytes) VALUES (?, ?)",
              key,
              bytes,
            ));
      },
    };
    let out: T;
    await d.withTransactionAsync(async () => {
      out = await fn(tx);
    });
    return out!;
  });
  last = run.catch(() => {});
  return run;
}
