// @pxl/client/index.ts

import "./src/page.ts";
import "./src/boot.ts";

import { registerRootComponent } from "expo";

import * as locale from "@/ui/shared/locale/i18n.ts";
import { App } from "./src/app.tsx";

locale.boot();
registerRootComponent(App);
