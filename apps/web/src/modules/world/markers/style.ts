// @pxl/web/modules/world/markers/style.ts

export const marker_colors = {
  /** Select and move: the pointer's own highlight, and the location puck's
   * interaction zone. Reserved for "you could grab this", never for a
   * preview of a change. */
  ACTION: "#4fd1c5",
  /** Removal, where the thing being removed cannot be drawn as itself —
   * a decor silhouette whose model has not arrived yet. Dark rather than
   * ACTION's cyan, which a user reads as "move" on sight. */
  ERASE: "#1d2026",
} as const;
