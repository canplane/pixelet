// @pxl/client/index.ts

import "./src/page.ts";
import "./src/boot.ts";

import { registerRootComponent } from "expo";

import * as locale from "@/ui/locale/i18n.ts";
import { App } from "./src/app.tsx";

// The UI's words are set up before the UI, which reads where the boot
// (src/boot.ts) stands.
locale.boot();
registerRootComponent(App);
