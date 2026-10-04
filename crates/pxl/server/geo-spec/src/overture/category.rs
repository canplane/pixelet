// pxl-geo-spec/overture/category.rs

pub fn map(category: &str) -> Option<&'static str> {
    OVERTURE_CATEGORY_MAP.binary_search_by_key(&category, |(key, _)| *key).ok().map(|at| OVERTURE_CATEGORY_MAP[at].1)
}

// Overture's `taxonomy` categories to a key of spec/pxl/spec/bake/poi_badge.json,
// sorted for binary search. A category under one of these takes it by its
// hierarchy, so only the one a kind starts at is named. Each side must cover
// the other (tested); an unknown target silently draws the default badge.
pub static OVERTURE_CATEGORY_MAP: [(&str, &str); 46] = [
    ("atm", "atm"),
    ("bank", "bank"),
    ("bar", "bar"),
    ("bed_and_breakfast", "guest_house"),
    ("brewery", "bar"),
    ("bus_station", "bus_station"),
    ("cafe", "cafe"),
    ("chicken_restaurant", "fast_food"),
    ("clothing_store", "clothes"),
    ("coffee_shop", "cafe"),
    ("college_university", "university"),
    ("convenience_store", "convenience"),
    ("dental_clinic", "dentist"),
    ("department_store", "department_store"),
    ("elementary_school", "school"),
    ("fast_food_restaurant", "fast_food"),
    ("fire_station", "fire_station"),
    ("gas_station", "fuel"),
    ("government_office", "townhall"),
    ("grocery_store", "supermarket"),
    ("guest_house", "guest_house"),
    ("high_school", "school"),
    ("historic_site", "attraction"),
    ("hospital", "hospital"),
    ("hotel", "hotel"),
    ("korean_restaurant", "restaurant"),
    ("library", "library"),
    ("metro_station", "subway"),
    ("middle_school", "school"),
    ("motel", "hotel"),
    ("museum", "museum"),
    ("national_park", "park"),
    ("outpatient_care_facility", "clinic"),
    ("park", "park"),
    ("parking", "parking"),
    ("pharmacy", "pharmacy"),
    ("place_of_worship", "place_of_worship"),
    ("police_station", "police"),
    ("post_office", "post_office"),
    ("preschool", "kindergarten"),
    ("pub", "pub"),
    ("restaurant", "restaurant"),
    ("sandwich_shop", "restaurant"),
    ("school", "school"),
    ("shopping_mall", "mall"),
    ("train_station", "station"),
];

#[cfg(test)]
#[path = "category.test.rs"]
mod test;
