// @pxl/build/stamp.ts

/**
 * What a build is: when it was made, from which source, and under which
 * number. The bundle carries it (`virtual:pxl-build`, which the app's
 * build.ts reads), and the web export states it at /build.json. The source
 * is git's short hash, with `-dirty` when the tree had changes. The number is
 * `#<n>` when CI handed the build one (PXL_BUILD_NUMBER). Otherwise it is
 * `#<n>-<id>`, from the number the deployment serves (ts/client/.build_number,
 * kept by script/build-number.py), or the id alone before any number is
 * cached.
 */

import { execSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import process from "node:process";
import { REPO, WORKSPACE } from "./path.ts";

export interface Stamp {
  time: string;
  id: string;
  number: string;
  /** CI's number, or null for a build CI did not number. */
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
    /* The tree is not a git checkout. */
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
      /* No number the deployment serves is cached yet. */
    }
  }
  return {
    time: new Date().toISOString(),
    id,
    number,
    official: official ? Number(official) : null,
  };
}
