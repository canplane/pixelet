// @xpute/example/web/core.worker.ts

/** The worker the page starts: it serves the core. */

import { serve } from "@xpute/runtime";
import { manifest } from "./core.ts";

void serve(manifest);
