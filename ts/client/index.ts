// @pxl/client/index.ts

import "./src/page.ts";
import "./src/boot.ts";

import { registerRootComponent } from "expo";

import * as locale from "@/ui/locale/i18n.ts";
import { App } from "./src/app.tsx";

// The UI's words, then the UI, which reads where the boot stands.
locale.boot();
registerRootComponent(App);
