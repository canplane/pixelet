// @pxl/native/index.ts

import { registerRootComponent } from "expo";

import * as bracket from "@pxl/client/bracket.tsx";
import * as locale from "@pxl/client/shell/locale/i18n.ts";
import { boot } from "@pxl/client/abi/boot.ts";
import { app } from "./src/bracket.ts";
import { App } from "./src/app.tsx";

bracket.set(app);
// The shell's words first, then the client, bottom up; the shell reads where it stands.
locale.boot();
boot();
registerRootComponent(App);
