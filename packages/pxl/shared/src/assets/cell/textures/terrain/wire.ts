// @pxl/shared/assets/cell/textures/terrain/wire.ts
//
// Bridges the real terrain PNGs in this same textures/ dir to typed URLs —
// not to be confused with spec/cell/cube.ts, which defines the numeric
// cube_types/cube_colors identity these images just decorate.
// Provenance: tools/manifest.json, downloaded/normalized by
// ./tools/download_texture_pack.ts.
import type { ChannelMap } from "@pxl/shared/spec/cell/type.ts";

import land_bare from "./land_bare.png";
import land_sea from "./land_sea.png";
import land_green_deep from "./land_green_deep.png";
import land_green_light from "./land_green_light.png";
import land_sand from "./land_sand.png";
import land_residential from "./land_residential.png";
import land_commercial from "./land_commercial.png";
import land_industrial from "./land_industrial.png";
import land_sports from "./land_sports.png";
import land_education from "./land_education.png";
import land_medical from "./land_medical.png";
import land_public from "./land_public.png";
import land_apron from "./land_apron.png";
import land_helipad from "./land_helipad.png";
import water_river from "./water_river.png";
import water_lake from "./water_lake.png";
import water_wetland from "./water_wetland.png";
import structure_building from "./structure_building.png";
import structure_building_station from "./structure_building_station.png";
import structure_building_medical from "./structure_building_medical.png";
import structure_building_education from "./structure_building_education.png";
import structure_building_public from "./structure_building_public.png";
import transport_road_default from "./transport_road_default.png";
import transport_road_motorway from "./transport_road_motorway.png";
import transport_road_primary from "./transport_road_primary.png";
import transport_road_secondary from "./transport_road_secondary.png";
import transport_pedestrian from "./transport_pedestrian.png";
import transport_parking from "./transport_parking.png";
import transport_rail_main from "./transport_rail_main.png";
import transport_rail_subway from "./transport_rail_subway.png";
import transport_runway from "./transport_runway.png";
import transport_taxiway from "./transport_taxiway.png";
import side_bare from "./side_bare.png";
import side_green_deep from "./side_green_deep.png";
import side_green_light from "./side_green_light.png";
import side_sports from "./side_sports.png";
import side_sand from "./side_sand.png";
import side_residential from "./side_residential.png";
import side_commercial from "./side_commercial.png";
import side_industrial from "./side_industrial.png";
import side_education from "./side_education.png";
import side_medical from "./side_medical.png";
import side_public from "./side_public.png";
import side_apron from "./side_apron.png";
import side_helipad from "./side_helipad.png";

export default {
  // ch_key -> type_key -> texture URL.
  top: {
    land: {
      BARE: { url: land_bare },
      SEA: { url: land_sea },
      GREEN_DEEP: { url: land_green_deep },
      GREEN_LIGHT: { url: land_green_light },
      SAND: { url: land_sand },
      RESIDENTIAL: { url: land_residential },
      COMMERCIAL: { url: land_commercial },
      INDUSTRIAL: { url: land_industrial },
      SPORTS: { url: land_sports },
      EDUCATION: { url: land_education },
      MEDICAL: { url: land_medical },
      PUBLIC: { url: land_public },
      APRON: { url: land_apron },
      HELIPAD: { url: land_helipad },
    },
    water: {
      RIVER: { url: water_river },
      LAKE: { url: water_lake },
      WETLAND: { url: water_wetland },
    },
    structure: {
      BUILDING: { url: structure_building },
      BUILDING_STATION: { url: structure_building_station },
      BUILDING_MEDICAL: { url: structure_building_medical },
      BUILDING_EDUCATION: { url: structure_building_education },
      BUILDING_PUBLIC: { url: structure_building_public },
    },
    transport: {
      ROAD_DEFAULT: { url: transport_road_default },
      ROAD_MOTORWAY: { url: transport_road_motorway },
      ROAD_PRIMARY: { url: transport_road_primary },
      ROAD_SECONDARY: { url: transport_road_secondary },
      PEDESTRIAN: { url: transport_pedestrian },
      PARKING: { url: transport_parking },
      RAIL_MAIN: { url: transport_rail_main },
      RAIL_SUBWAY: { url: transport_rail_subway },
      RUNWAY: { url: transport_runway },
      TAXIWAY: { url: transport_taxiway },
    },
  } satisfies ChannelMap,

  // Must be fully opaque — shader only reads `.rgb`.
  side: {
    land: {
      BARE: { url: side_bare },
      GREEN_DEEP: { url: side_green_deep },
      GREEN_LIGHT: { url: side_green_light },
      SPORTS: { url: side_sports },
      SAND: { url: side_sand },
      RESIDENTIAL: { url: side_residential },
      COMMERCIAL: { url: side_commercial },
      INDUSTRIAL: { url: side_industrial },
      EDUCATION: { url: side_education },
      MEDICAL: { url: side_medical },
      PUBLIC: { url: side_public },
      APRON: { url: side_apron },
      HELIPAD: { url: side_helipad },
    },
  } satisfies Partial<ChannelMap>,
};
