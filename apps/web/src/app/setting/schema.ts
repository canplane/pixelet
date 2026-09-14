// @pxl/web/app/setting/schema.ts

/**
 * Every setting the app has, declared once: its default, its kind, and
 * where the settings sheet shows it. The store (store.ts) reads
 * the defaults from here and the sheet (section.tsx) renders
 * rows from here, so adding a setting is one entry in SETTINGS plus its
 * two strings in the locale catalogue under `settings.<section>.<key>`.
 *
 * The values are the fallback layer of a two-layer model, the way an
 * editor keeps a default settings file under the user's own: what the app
 * reads is the defaults with the user's overrides on top, and only the
 * overrides are stored. See store.ts for why that matters.
 *
 * A `dev` setting exists in every build — the store is the same — but is
 * only offered in the sheet under `__IS_DEV__`, and its strings live in
 * the dev locale catalogue, so a production bundle carries neither the row
 * nor its name.
 */

import { DEFAULT_PERSON_VARIANT_KEY, DEFAULT_PET_VARIANT_KEY } from "@/module/world/character/variant.ts";

/** Rough per-frame cost, shown as a badge on the row. */
export type SettingCost = "low" | "med" | "high";

/** The sheet's sections, in the order they are shown. `character` is not a
 * section of the sheet at all — its two settings are picked from the
 * drawer's Character tab (character/character_picker.tsx). */
export type SettingSection = "display" | "camera" | "editing" | "lighting" | "quality" | "debug" | "character";

type Common = { section: SettingSection; cost?: SettingCost; dev?: boolean };
export type ToggleSetting = Common & { kind: "toggle"; default: boolean };
export type ChoiceSetting<T extends string = string> = Common & { kind: "choice"; default: T; options: readonly T[] };
export type TextSetting = Common & { kind: "text"; default: string };
export type Setting = ToggleSetting | ChoiceSetting | TextSetting;

const toggle = (section: SettingSection, dflt: boolean, extra: Omit<Common, "section"> = {}): ToggleSetting => ({
  kind: "toggle",
  default: dflt,
  section,
  ...extra,
});
const choice = <const T extends string>(
  section: SettingSection,
  options: readonly T[],
  dflt: NoInfer<T>,
  extra: Omit<Common, "section"> = {},
): ChoiceSetting<T> => ({
  kind: "choice",
  default: dflt,
  options,
  section,
  ...extra,
});
const text = (section: SettingSection, dflt: string): TextSetting => ({ kind: "text", default: dflt, section });

/**
 * What terrain does to the camera (camera/terrain_gate.ts).
 *
 *   through — nothing. The eye goes wherever the controls put it, ground
 *             included, where the terrain shell has no inward-facing
 *             geometry and the scene reads inside out.
 *   stop    — a control goes as far as it asked or as far as it can.
 *             Nothing moves on its own; a zoom into a hill stops short.
 *   climb   — dist is always granted and the *view* pitches up instead,
 *             as far as straight down, so every zoom level stays
 *             reachable at the cost of an angle nobody asked for.
 */
export const CAMERA_TERRAIN_MODES = ["through", "stop", "climb"] as const;
export type CameraTerrainMode = (typeof CAMERA_TERRAIN_MODES)[number];

export const SETTINGS = {
  // ---- display: what the world shows. Turning one off removes information
  // — a badge, a scattered tree, a building — which is what separates these
  // from lighting and quality, which leave the same world drawn differently.
  /** The index sweep, the badge draws and the atlas: a couple of ms a
   * frame while places are being placed, nothing once they are. */
  show_poi: toggle("display", true, { cost: "low" }),
  /** Procedurally scattered decor only; what the player placed stays. */
  show_decor: toggle("display", true, { cost: "low" }),
  /** Whether the STRUCTURE ground channel is painted. The bake writes it
   * either way (overture_policy.ts); this is a render decision. */
  show_structures: toggle("display", false, { dev: true }),

  // ---- camera: how the view moves. Nothing here changes what is drawn.
  /** Rotate-drag and pinch stop writing polar; azimuth is unaffected.
   * Off by default since 2026-09-13 — the tilt is the one control a map
   * view is expected to have, and locking it read as the app being stuck. */
  lock_camera_tilt: toggle("camera", false),
  // climb, because the one thing a map camera must not do is refuse to
  // move. stop is the more literal answer and the worse default: it hands
  // back a control that did nothing, which reads as the app being stuck
  // rather than as the ground being in the way.
  camera_terrain: choice("camera", CAMERA_TERRAIN_MODES, "climb"),
  /** Panning re-samples the terrain height under the map centre each step. */
  ground_snap: toggle("camera", true, { dev: true }),

  // ---- editing: what the tools may change.
  /** The eraser digs into the baked terrain where nothing is placed. Off by
   * default: the terrain is real map data, not the player's scratch canvas. */
  enable_terrain_carving: toggle("editing", false),

  // ---- lighting: how readable the same geometry is. Cheapest first; the
  // dev ones at the bottom are the passes not yet decided on.
  /** Darkens cube edges that border open air (render/gpu/chunk_shader.ts).
   * A few fragment ops behind a uniform; the shader is the same either way. */
  show_edge_shade: toggle("lighting", true, { cost: "low" }),
  /** A uniform on an already-compiled shader term; not a pass. */
  show_rim_light: toggle("lighting", true, { cost: "low" }),

  // ---- quality.
  /** Ground colour from the art pack (four sampler2DArray taps per fragment)
   * instead of the flat per-instance average. The shader's `u_low_spec` is
   * this, inverted. */
  texture_pack: toggle("quality", true, { cost: "med" }),

  // ---- debug: diagnostics, dev builds only. Off by default so they are
  // never in the way while eyeballing real rendering.
  /** world.tsx's DebugLayer gizmos: frame origins, page corners, map centre. */
  show_debug_objects: toggle("debug", false, { dev: true }),
  /** The sched/ledger.ts corner panel. The console stream is a button on it. */
  show_ledger: toggle("debug", false, { dev: true }),
  /** LOD cross-fade per fragment instead of per chunk: both tiers draw with
   * complementary dither and dissolve, at the cost of drawing a blending
   * chunk twice and a pattern that crawls without temporal AA. */
  pixel_dither: toggle("debug", false, { dev: true }),

  // ---- character: picked in the drawer, not the sheet.
  person_variant_key: text("character", DEFAULT_PERSON_VARIANT_KEY),
  pet_variant_key: text("character", DEFAULT_PET_VARIANT_KEY),
} as const satisfies Record<string, Setting>;

export type SettingKey = keyof typeof SETTINGS;
export type SettingValues = { [K in SettingKey]: (typeof SETTINGS)[K]["default"] };
export type ToggleKey = { [K in SettingKey]: (typeof SETTINGS)[K] extends ToggleSetting ? K : never }[SettingKey];

export const SETTING_KEYS = Object.keys(SETTINGS) as SettingKey[];

/** The fallback layer: every setting at its declared default. */
export const DEFAULT_SETTINGS: Readonly<SettingValues> = Object.fromEntries(
  SETTING_KEYS.map((key) => [key, SETTINGS[key].default]),
) as SettingValues;

/** The settings shown under a section, in declaration order. */
export function settings_in(section: SettingSection): SettingKey[] {
  return SETTING_KEYS.filter((key) => SETTINGS[key].section === section);
}

/** Whether `value` is something `key` may hold — what a stored override is
 * checked against before it is trusted. */
export function is_valid_setting(key: SettingKey, value: unknown): value is SettingValues[SettingKey] {
  const setting: Setting = SETTINGS[key];
  switch (setting.kind) {
    case "toggle":
      return typeof value === "boolean";
    case "choice":
      return typeof value === "string" && setting.options.includes(value);
    case "text":
      return typeof value === "string";
  }
}
