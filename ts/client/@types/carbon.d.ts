// @pxl/client/@types/carbon.d.ts

// An icon of IBM's Carbon as @carbon/icons lays it out at its 32 grid
// (ui/shared/component/icon.tsx draws it), which the package ships no types
// for: an SVG's view box and the paths and circles in it.
declare module "@carbon/icons/es/*/32.js" {
  const icon: {
    attrs: { viewBox: string };
    content: readonly (
      | { elem: "path"; attrs: { d: string; transform?: string } }
      | {
          elem: "circle";
          attrs: { cx: string; cy: string; r: string; fill?: string };
        }
    )[];
  };
  export default icon;
}
