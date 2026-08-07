// @pxl/api/app/resources.ts

import { Pool } from "@db/postgres";

import type { i32 } from "@xpute/core/kit/word.ts";

import * as env from "@/lib/env/mod.ts";

export type AppResources = {
  main_pool: Pool;
  geo_pool: Pool;

  disposers: (() => void | Promise<void>)[];
  dispose: () => Promise<void> | void;
};

type PostgresConfig = {
  main_url: string;
  geo_url: string;

  pool_size: i32;
  lazy: 0 | 1; // postgres Pool constructor's 3rd arg
};

async function probe_pool(name: string, pool: Pool, sql: string): Promise<void> {
  const client = await pool.connect();
  try {
    await client.queryArray(sql);
    console.debug(`db: ${name}: ok`);
  } finally {
    client.release();
  }
}

export async function make_app_resrcs(): Promise<AppResources> {
  const cfg: PostgresConfig = {
    main_url: env.must("DB_MAIN_URL"),
    geo_url: env.must("DB_GEO_URL"),

    pool_size: env.must_int("DB_POOL_SIZE", { fallback: 8, min: 1 }) as i32,
    lazy: 1,
  };

  const main_pool = new Pool(cfg.main_url, cfg.pool_size, !!cfg.lazy);
  await probe_pool("main", main_pool, "select 1");

  const geo_pool = new Pool(cfg.geo_url, cfg.pool_size, !!cfg.lazy);
  await probe_pool("geo", geo_pool, "select postgis_full_version()");

  const disposers: (() => void | Promise<void>)[] = [];
  // the close API can vary by postgres library version — call end() if
  // this Pool implementation provides it.
  async function dispose(): Promise<void> {
    for (let i = disposers.length - 1; i >= 0; i--) {
      try {
        await disposers[i]();
      } catch {}
    }
    // best-effort
    try {
      // deno-postgres / postgres libraries usually expose end() or close()
      await main_pool.end?.();
    } catch {}
    try {
      await geo_pool.end?.();
    } catch {}
  }

  return { main_pool, geo_pool, disposers, dispose };
}
