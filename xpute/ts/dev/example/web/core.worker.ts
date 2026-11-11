// @xpute/example/web/core.worker.ts

import { serve } from "@xpute/runtime";
import { MANIFEST } from "../manifest.spec.ts";
import { core } from "./core.ts";

void serve(MANIFEST, core);
