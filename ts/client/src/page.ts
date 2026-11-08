// @pxl/client/page.ts

/** What the page has and a device does not: the document the views are put
 * into (document.css), and, in a development bundle, the console forwarded
 * to the dev server (remote-console.ts). */

import "./document.css";
import { install } from "./remote-console.ts";

if (__DEV_BUNDLE__) install();
