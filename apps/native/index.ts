// @pxl/native/index.ts

// The build's constants before anything reads them.
import "./src/globals.ts";

import { registerRootComponent } from "expo";

import { App } from "./src/app.tsx";

registerRootComponent(App);
