// @pxl/native/platform/glass.android.tsx

// Android draws the page's own (glass.tsx is an iPhone's): its system
// controls are Material's, which read as any Android app's rather than this
// one's, and it has no backdrop blur to lend a surface over the map.
export * from "../../../web/src/platform/web/glass.tsx";
