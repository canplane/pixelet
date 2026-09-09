// @pxl/api/bake/overture/model/category_map.ts

/**
 * Overture's Places taxonomy (docs.overturemaps.org/schema/reference/places/)
 * has thousands of `categories.primary` values — this maps the common ones
 * to the vocabulary poi_theme.ts keys off, so the client-side theme table
 * doesn't need to know which provider is behind a given POI. An Overture
 * category with no entry here passes through unmapped and resolves to
 * poi_theme.ts's DEFAULT_THEME.
 *
 * Deliberately a starting set (the categories this session's own Gangnam/
 * Suwon spot-checks actually turned up plus the obvious common ones), not
 * an attempt to cover Overture's full taxonomy — extend as real gaps show
 * up rather than front-loading a guess at every possible value.
 */
export const OVERTURE_CATEGORY_MAP: Record<string, string> = {
  // health
  hospital: "hospital",
  medical_center: "hospital",
  clinic: "clinic",
  doctor: "clinic",
  pharmacy: "pharmacy",
  dentist: "dentist",
  // food & drink
  korean_restaurant: "restaurant",
  restaurant: "restaurant",
  sandwich_shop: "restaurant",
  chicken_restaurant: "fast_food",
  fast_food_restaurant: "fast_food",
  cafe: "cafe",
  coffee_shop: "cafe",
  bar: "bar",
  pub: "pub",
  brewery: "bar",
  // money
  bank: "bank",
  atm: "atm",
  // education
  school: "school",
  elementary_school: "school",
  middle_school: "school",
  high_school: "school",
  university: "university",
  college: "college",
  kindergarten: "kindergarten",
  preschool: "kindergarten",
  library: "library",
  // shopping
  supermarket: "supermarket",
  grocery_store: "supermarket",
  convenience_store: "convenience",
  shopping_mall: "mall",
  department_store: "department_store",
  clothing_store: "clothes",
  // transport
  subway_station: "subway",
  train_station: "station",
  metro_station: "subway",
  bus_station: "bus_station",
  bus_stop: "bus_station",
  gas_station: "fuel",
  fuel_station: "fuel",
  parking: "parking",
  parking_garage: "parking",
  // lodging & leisure
  hotel: "hotel",
  motel: "hotel",
  guest_house: "guest_house",
  bed_and_breakfast: "guest_house",
  park: "park",
  national_park: "park",
  museum: "museum",
  tourist_attraction: "attraction",
  landmark_and_historical_building: "attraction",
  // religion
  church_cathedral: "place_of_worship",
  place_of_worship: "place_of_worship",
  temple: "place_of_worship",
  mosque: "place_of_worship",
  synagogue: "place_of_worship",
  // civic
  police_station: "police",
  fire_station: "fire_station",
  post_office: "post_office",
  city_hall: "townhall",
  government_office: "townhall",
};
