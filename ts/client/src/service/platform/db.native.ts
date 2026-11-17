// @pxl/client/service/platform/db.native.ts

import type { Transaction } from "./types.ts";
import { openDatabaseAsync } from "expo-sqlite";

const db = openDatabaseAsync("pxl.db").then(async (database) => {
  await database.execAsync(
    "CREATE TABLE IF NOT EXISTS file (key TEXT PRIMARY KEY, bytes BLOB NOT NULL) WITHOUT ROWID",
  );
  return database;
});

/** The last transaction asked for, settled. Transactions run one after
 * another, because a SQLite transaction takes in every query made while it
 * is open, even one meant for another transaction. */
let last: Promise<unknown> = db.catch(() => {});

export function transaction<T>(
  fn: (tx: Transaction) => Promise<T>,
): Promise<T> {
  const run = last.then(async () => {
    const database = await db;
    const tx: Transaction = {
      get: async (key) => {
        const row = await database.getFirstAsync<{
          bytes: Uint8Array<ArrayBuffer>;
        }>("SELECT bytes FROM file WHERE key = ?", key);
        return row?.bytes ?? null;
      },
      put: async (key, bytes) => {
        await (bytes === null
          ? database.runAsync("DELETE FROM file WHERE key = ?", key)
          : database.runAsync(
              "INSERT OR REPLACE INTO file (key, bytes) VALUES (?, ?)",
              key,
              bytes,
            ));
      },
    };
    let out: T;
    await database.withTransactionAsync(async () => {
      out = await fn(tx);
    });
    return out!;
  });
  last = run.catch(() => {});
  return run;
}
