// @pxl/client/index.ts

import "./src/page.ts";
import "./src/boot.ts";

import { registerRootComponent } from "expo";

import * as locale from "@/shell/locale/i18n.ts";
import { App } from "./src/app.tsx";

// The shell's words, then the shell, which reads where the boot stands.
locale.boot();
registerRootComponent(App);
