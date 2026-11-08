// @xpute/example/web/core.worker.ts

import { serve } from "@xpute/runtime";
import { manifest } from "./core.ts";

void serve(manifest);
