// @pxl/build/stamp.ts

/**
 * What a build is — when, from which source, under which number — which the
 * bundle carries (`virtual:pxl-build`, deploy.ts reads it) and the page's
 * export states at /build.json. The source is git's short hash, `-dirty` where
 * the tree had changes; the number is `#<n>` where CI handed the build one
 * (PXL_BUILD_NUMBER), or else `#<n>-<id>` from the number the deployment
 * serves (ts/client/.build_number, script/build-number.py), or the id alone
 * before any is cached.
 */

import { execSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import process from "node:process";
import { REPO, WORKSPACE } from "./paths.ts";

export interface Stamp {
  time: string;
  id: string;
  number: string;
  /** CI's number, none for a build CI did not number. */
  official: number | null;
}

export function stamp(): Stamp {
  let id = "unknown";
  try {
    const git = (args: string) =>
      execSync(`git ${args}`, {
        cwd: REPO,
        stdio: ["ignore", "pipe", "ignore"],
      })
        .toString()
        .trim();
    id = `${git("rev-parse --short HEAD")}${
      git("status --porcelain") !== "" ? "-dirty" : ""
    }`;
  } catch {
    /* not a checkout */
  }
  const official = process.env.PXL_BUILD_NUMBER;
  let number = id;
  if (official) number = `#${official}`;
  else {
    try {
      const last = fs
        .readFileSync(path.join(WORKSPACE, "client/.build_number"), "utf8")
        .trim();
      if (/^\d+$/.test(last)) number = `#${last}-${id}`;
    } catch {
      /* none seen yet */
    }
  }
  return {
    time: new Date().toISOString(),
    id,
    number,
    official: official ? Number(official) : null,
  };
}
