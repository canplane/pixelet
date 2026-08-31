// @pxl/shared/assets/decor/trains.ts

// Same bridging role as ./trees.ts — see that file's own doc comment.
// Curated to single locomotive/railcar models, not connected multi-car
// consists: this pipeline scatters one independent instance per eligible
// cell (see scatter.ts's own doc comment), so "an occasional train sits
// here" is the honest read, not "a full train runs along these tracks".
import locomotive_a_url from "./kenney_train_kit/train-locomotive-a.glb?url";
import locomotive_b_url from "./kenney_train_kit/train-locomotive-b.glb?url";
import diesel_a_url from "./kenney_train_kit/train-diesel-a.glb?url";
import electric_city_a_url from "./kenney_train_kit/train-electric-city-a.glb?url";
import train_skin_url from "./kenney_train_kit/colormap.png";

export const TRAIN_SKIN_URL: string = train_skin_url;

export const TRAIN_MODEL_URLS: readonly string[] = [
  locomotive_a_url,
  locomotive_b_url,
  diesel_a_url,
  electric_city_a_url,
];
