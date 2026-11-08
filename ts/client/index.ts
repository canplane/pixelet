// @pxl/client/index.ts

import "./src/app/page.ts";
import "./src/app/boot.ts";

import { registerRootComponent } from "expo";

import * as locale from "@/ui/locale/i18n.ts";
import { App } from "./src/app/app.tsx";

// The UI's words are set up before the UI, which reads where the boot
// (src/app/boot.ts) stands.
locale.boot();
registerRootComponent(App);
