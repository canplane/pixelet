// pxl-geo/search/category.rs

/// Overture's Places taxonomy (docs.overturemaps.org/schema/reference/places/)
/// has thousands of `categories.primary` values — this maps the common ones to
/// the vocabulary the client's POI theme keys off, so that table does not need
/// to know which provider is behind a given POI. An Overture category with no
/// entry here passes through unmapped and resolves to the theme's default.
///
/// Deliberately a starting set (the categories Gangnam/Suwon spot-checks
/// actually turned up plus the obvious common ones), not an attempt to cover
/// Overture's full taxonomy — extend as real gaps show up rather than
/// front-loading a guess at every possible value.
///
/// The POI bake wants the same table, and when it is ported this is the piece
/// that decides where an Overture vocabulary shared by two crates lives.
pub fn map(primary: &str) -> Option<&'static str> {
    Some(match primary {
        // health
        "hospital" | "medical_center" => "hospital",
        "clinic" | "doctor" => "clinic",
        "pharmacy" => "pharmacy",
        "dentist" => "dentist",
        // food & drink
        "korean_restaurant" | "restaurant" | "sandwich_shop" => "restaurant",
        "chicken_restaurant" | "fast_food_restaurant" => "fast_food",
        "cafe" | "coffee_shop" => "cafe",
        "bar" | "brewery" => "bar",
        "pub" => "pub",
        // money
        "bank" => "bank",
        "atm" => "atm",
        // education
        "school" | "elementary_school" | "middle_school" | "high_school" => "school",
        "university" => "university",
        "college" => "college",
        "kindergarten" | "preschool" => "kindergarten",
        "library" => "library",
        // shopping
        "supermarket" | "grocery_store" => "supermarket",
        "convenience_store" => "convenience",
        "shopping_mall" => "mall",
        "department_store" => "department_store",
        "clothing_store" => "clothes",
        // transport
        "subway_station" | "metro_station" => "subway",
        "train_station" => "station",
        "bus_station" | "bus_stop" => "bus_station",
        "gas_station" | "fuel_station" => "fuel",
        "parking" | "parking_garage" => "parking",
        // lodging & leisure
        "hotel" | "motel" => "hotel",
        "guest_house" | "bed_and_breakfast" => "guest_house",
        "park" | "national_park" => "park",
        "museum" => "museum",
        "tourist_attraction" | "landmark_and_historical_building" => "attraction",
        // religion
        "church_cathedral" | "place_of_worship" | "temple" | "mosque" | "synagogue" => "place_of_worship",
        // civic
        "police_station" => "police",
        "fire_station" => "fire_station",
        "post_office" => "post_office",
        "city_hall" | "government_office" => "townhall",
        _ => return None,
    })
}
