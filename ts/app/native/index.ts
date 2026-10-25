// @pxl/native/index.ts

import { registerRootComponent } from "expo";

import * as bracket from "@pxl/client/bracket.tsx";
import { boot } from "@pxl/client/shell/boot.ts";
import { app } from "./src/bracket.ts";
import { App } from "./src/app.tsx";

bracket.set(app);
boot();
registerRootComponent(App);
