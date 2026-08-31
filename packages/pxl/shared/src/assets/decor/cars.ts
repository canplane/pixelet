// @pxl/shared/assets/decor/cars.ts

// Same bridging role as ./trees.ts — see that file's own doc comment.
// Curated to ordinary everyday vehicles (a sedan, an SUV, a van, a sports
// hatchback, a taxi) — the car-kit's emergency/utility/kart/race vehicles
// don't fit "ambient highway traffic" the way this kind uses them (see
// car_kind.ts's own doc comment).
import sedan_url from "./kenney_car_kit/sedan.glb?url";
import suv_url from "./kenney_car_kit/suv.glb?url";
import van_url from "./kenney_car_kit/van.glb?url";
import hatchback_sports_url from "./kenney_car_kit/hatchback-sports.glb?url";
import taxi_url from "./kenney_car_kit/taxi.glb?url";
import car_skin_url from "./kenney_car_kit/colormap.png";

export const CAR_SKIN_URL: string = car_skin_url;

export const CAR_MODEL_URLS: readonly string[] = [
  sedan_url,
  suv_url,
  van_url,
  hatchback_sports_url,
  taxi_url,
];
