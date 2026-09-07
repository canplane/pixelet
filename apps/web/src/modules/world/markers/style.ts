// @pxl/web/modules/world/markers/style.ts

export const marker_colors = {
  ACTION: "#4fd1c5",
  /** Eraser's own ghost when it's hovering bare terrain, about to dig into
   * it rather than remove an existing placed thing — hover_cursor.tsx
   * deliberately never borrows a real color_id/texture for that case
   * (there's no "real thing" a hole has a color of), so this is its whole
   * visual identity: dark enough to read as "about to become nothing"
   * rather than as another paintable swatch. */
  VOID: "#26262b",
} as const;
