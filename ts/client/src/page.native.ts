// @pxl/client/page.native.ts

/** The native app has no document to style, and keeps its own console
 * (page.web.ts). What it needs first is Expo's globals, among them the text
 * codecs, which xpute's kit reads as it loads. */

import "expo";
