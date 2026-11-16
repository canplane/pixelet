// @pxl/client/abi/scene.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type SceneContent =
  typeof SCENE_CONTENT_WORLD | typeof SCENE_CONTENT_CHARACTER;
export const SCENE_CONTENT_WORLD = 0;
export const SCENE_CONTENT_CHARACTER = 1;

/** The member `val` is, or null for a number no member has. */
export function of_SceneContent(val: number): SceneContent | null {
  return val === SCENE_CONTENT_WORLD || val === SCENE_CONTENT_CHARACTER
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_SceneContent(val: SceneContent): string {
  switch (val) {
    case SCENE_CONTENT_WORLD:
      return "World";
    case SCENE_CONTENT_CHARACTER:
      return "Character";
  }
}
