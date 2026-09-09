// @pxl/web/modules/world/poi/poi_theme.ts

/**
 * Category -> icon/color lookup for poi_layer.tsx's badge — the one visual
 * idea worth keeping from the pre-DoD POI implementation
 * (`checkpoint/20260107-float`'s `entities/geo/ui/pois/poi-theme-config.ts`)
 * now that POI is back (2026-09-07, see
 * discussions/20260907-world-rendering-tiers.md). That version's own
 * day/night variants and hover glow aren't ported here — this pass only
 * brings back "a POI reads as its own kind at a glance," not the full
 * interactive badge; see poi_layer.tsx's own top doc comment for what's
 * still deferred.
 *
 * Keyed off `POI.category` (overture_provider.ts's `category_of()`, which
 * maps Overture's `categories.primary` through OVERTURE_CATEGORY_MAP into
 * this table's own vocabulary), falling back to DEFAULT_THEME for anything
 * not explicitly listed rather than guessing a mapping.
 *
 * `icon` is a plain emoji — tried swapping these for tinted Lucide glyphs
 * (2026-09-08) and reverted: at this badge's actual on-screen size a thin
 * stroked icon reads far less recognizably than an emoji's own dense,
 * multi-shape glyph does, and that shape-level legibility (not stroke
 * weight, tried thickening it first) was the real problem.
 */

export type PoiTheme = { icon: string; color: string };

const CATEGORY_THEME: Record<string, PoiTheme> = {
  // health
  hospital: { icon: "🏥", color: "#e57373" },
  clinic: { icon: "🏥", color: "#e57373" },
  pharmacy: { icon: "💊", color: "#e57373" },
  dentist: { icon: "🦷", color: "#e57373" },
  // food & drink — restaurant/fast_food/bar used to all share this exact
  // amber; fine with the emoji's own glyph carrying enough identity on its
  // own, but worth keeping separated regardless of icon style.
  restaurant: { icon: "🍴", color: "#ffb74d" },
  cafe: { icon: "☕", color: "#a1887f" },
  fast_food: { icon: "🍔", color: "#ff8a65" },
  bar: { icon: "🍺", color: "#f06292" },
  pub: { icon: "🍺", color: "#f06292" },
  // money
  bank: { icon: "🏦", color: "#64b5f6" },
  atm: { icon: "🏧", color: "#64b5f6" },
  // education
  school: { icon: "🏫", color: "#81c784" },
  university: { icon: "🎓", color: "#81c784" },
  college: { icon: "🎓", color: "#81c784" },
  kindergarten: { icon: "🧸", color: "#81c784" },
  library: { icon: "📚", color: "#81c784" },
  // shopping
  supermarket: { icon: "🛒", color: "#9575cd" },
  convenience: { icon: "🏪", color: "#9575cd" },
  mall: { icon: "🛍️", color: "#9575cd" },
  department_store: { icon: "🛍️", color: "#9575cd" },
  clothes: { icon: "👕", color: "#9575cd" },
  // transport
  subway: { icon: "🚉", color: "#fdd835" },
  station: { icon: "🚉", color: "#fdd835" },
  bus_station: { icon: "🚌", color: "#fdd835" },
  fuel: { icon: "⛽", color: "#90a4ae" },
  parking: { icon: "🅿️", color: "#90a4ae" },
  // lodging & leisure
  hotel: { icon: "🏨", color: "#4dd0e1" },
  guest_house: { icon: "🏨", color: "#4dd0e1" },
  park: { icon: "🌳", color: "#66bb6a" },
  museum: { icon: "🏛️", color: "#ba68c8" },
  attraction: { icon: "🏛️", color: "#ba68c8" },
  place_of_worship: { icon: "⛪", color: "#b0bec5" },
  // civic
  police: { icon: "🚓", color: "#7986cb" },
  fire_station: { icon: "🚒", color: "#7986cb" },
  post_office: { icon: "📮", color: "#7986cb" },
  townhall: { icon: "🏛️", color: "#7986cb" },
};

const DEFAULT_THEME: PoiTheme = { icon: "📍", color: "#90a4ae" };

export function resolve_poi_theme(category: string): PoiTheme {
  return CATEGORY_THEME[category] ?? DEFAULT_THEME;
}
