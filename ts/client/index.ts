// @pxl/client/index.ts

import { registerRootComponent } from "expo";

import "./src/page.ts";

import * as locale from "@/shell/locale/i18n.ts";
import { boot } from "@/abi/boot.ts";
import { App } from "./src/app.tsx";

// The shell's words first, then the client, bottom up; the shell reads where it stands.
locale.boot();
boot();
registerRootComponent(App);
