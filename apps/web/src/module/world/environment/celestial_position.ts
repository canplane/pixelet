// @pxl/web/module/world/environment/celestial_position.ts

/**
 * Real sun/moon direction and moon phase from the wall clock + the map's
 * actual latitude/longitude — replaces an earlier stylized version (a sine
 * elevation hump + linear azimuth sweep, capped low so the disc never left
 * the camera's own limited tilt range). That version deliberately traded
 * accuracy for guaranteed on-screen visibility; this one doesn't — the sun
 * can and does go outside the visible frustum near local noon (correct: you
 * don't look straight up at a map-view camera angle in real life either),
 * and rises/sets at the real compass bearing for wherever the map is
 * centered, not a fixed "always due east" stylization.
 *
 * Formulas are the standard "low precision" ones (Sun: USNO/NOAA low-
 * precision solar position, good to about 0.01deg; Moon: the commonly
 * published single-dominant-term ecliptic longitude/latitude approximation,
 * good to roughly 0.3-1deg — not a full perturbation series). Verified
 * against known behavior before wiring in: sunrise/sunset timing at the
 * equinox, solstice altitude swing, and the moon phase's day-over-day
 * angular rate (~12.2deg/day, matching the real synodic month) all came out
 * correct in a standalone check.
 *
 * UNITS: internally everything is degrees (matching how these formulas are
 * conventionally published) until the final conversion to a local direction
 * vector, which is where radians take over.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

const DEG2RAD: f64 = Math.PI / 180;
const RAD2DEG: f64 = 180 / Math.PI;

const norm360 = (deg: f64): f64 => ((deg % 360) + 360) % 360;

/** Julian Day (UT) from a JS Date — the standard epoch conversion (Unix epoch = JD 2440587.5). */
const to_julian_day = (date: Date): f64 => date.getTime() / 86400000 + 2440587.5;

/** Days since the J2000.0 epoch (JD 2451545.0) — the time base every formula below is expressed in. */
const days_since_j2000 = (date: Date): f64 => to_julian_day(date) - 2451545.0;

type EquatorialDeg = { ra: f64; dec: f64 };

/**
 * Sun's apparent geocentric right ascension/declination (degrees) — the
 * standard low-precision solar position formula (mean longitude + the two
 * largest equation-of-center terms; obliquity's own slow drift).
 */
function sun_equatorial(d: f64): EquatorialDeg & { obliquity: f64 } {
  const mean_long = norm360(280.46 + 0.9856474 * d);
  const mean_anomaly_rad = norm360(357.528 + 0.9856003 * d) * DEG2RAD;
  const ecliptic_long = norm360(
    mean_long + 1.915 * Math.sin(mean_anomaly_rad) + 0.02 * Math.sin(2 * mean_anomaly_rad),
  );
  const obliquity = 23.439 - 0.0000004 * d;

  const lon_rad = ecliptic_long * DEG2RAD;
  const obl_rad = obliquity * DEG2RAD;
  const ra = norm360(Math.atan2(Math.cos(obl_rad) * Math.sin(lon_rad), Math.cos(lon_rad)) * RAD2DEG);
  const dec = Math.asin(Math.sin(obl_rad) * Math.sin(lon_rad)) * RAD2DEG;
  return { ra, dec, obliquity };
}

/**
 * Moon's apparent geocentric right ascension/declination (degrees) — low
 * precision (single dominant periodic term per angle, ~0.3-1deg accuracy).
 * `obliquity` is threaded through from sun_equatorial so both bodies use
 * the exact same ecliptic-to-equatorial conversion for a given moment.
 */
function moon_equatorial(d: f64, obliquity: f64): EquatorialDeg & { ecliptic_long: f64 } {
  const mean_long = norm360(218.316 + 13.176396 * d);
  const mean_anomaly = norm360(134.963 + 13.064993 * d);
  const arg_latitude = norm360(93.272 + 13.22935 * d);

  const ecliptic_long = norm360(mean_long + 6.289 * Math.sin(mean_anomaly * DEG2RAD));
  const ecliptic_lat = 5.128 * Math.sin(arg_latitude * DEG2RAD);

  const lon_rad = ecliptic_long * DEG2RAD;
  const lat_rad = ecliptic_lat * DEG2RAD;
  const obl_rad = obliquity * DEG2RAD;

  const ra = norm360(
    Math.atan2(
      Math.sin(lon_rad) * Math.cos(obl_rad) - Math.tan(lat_rad) * Math.sin(obl_rad),
      Math.cos(lon_rad),
    ) * RAD2DEG,
  );
  const dec = Math.asin(
    Math.sin(lat_rad) * Math.cos(obl_rad) + Math.cos(lat_rad) * Math.sin(obl_rad) * Math.sin(lon_rad),
  ) * RAD2DEG;
  return { ra, dec, ecliptic_long };
}

/**
 * Equatorial (RA/Dec) -> horizontal (altitude/azimuth, both degrees) for an
 * observer at (lat, lon) in degrees (lon East-positive) at time `d` (days
 * since J2000). Azimuth is a real compass bearing: 0deg = North, 90deg =
 * East, measured clockwise — the classic Duffett-Smith construction (acos +
 * a sign flip from sin(hour_angle), rather than a single atan2 call) is
 * used deliberately: it's the version most consistently reproduced across
 * references, which matters more here than a marginally more elegant
 * formula that's easier to get a sign convention wrong on.
 */
function to_horizontal(eq: EquatorialDeg, lat: f64, lon: f64, d: f64): { alt: f64; az: f64 } {
  const gmst = norm360(280.46061837 + 360.98564736629 * d);
  const lst = norm360(gmst + lon);
  const hour_angle = (((lst - eq.ra + 180) % 360 + 360) % 360) - 180;

  const ha_rad = hour_angle * DEG2RAD;
  const dec_rad = eq.dec * DEG2RAD;
  const lat_rad = lat * DEG2RAD;

  const sin_alt = Math.sin(dec_rad) * Math.sin(lat_rad) + Math.cos(dec_rad) * Math.cos(lat_rad) * Math.cos(ha_rad);
  const alt_rad = Math.asin(sin_alt);

  const cos_az = Math.max(
    -1,
    Math.min(1, (Math.sin(dec_rad) - Math.sin(alt_rad) * Math.sin(lat_rad)) / (Math.cos(alt_rad) * Math.cos(lat_rad))),
  );
  let az_rad = Math.acos(cos_az);
  if (Math.sin(ha_rad) > 0) az_rad = 2 * Math.PI - az_rad;

  return { alt: alt_rad * RAD2DEG, az: az_rad * RAD2DEG };
}

/**
 * Compass azimuth (0=N, clockwise) + altitude -> this app's local (X,Y,Z)
 * direction, X=East/Y=North/Z=up (confirmed against the EPSG:3857/4326
 * adapters — no axis swap anywhere in that chain, and the scene is Z-up).
 * This is deliberately the *unrotated*, true-geographic direction: it feeds
 * SUN_DIR/sun_ref/moon_ref (real terrain lighting, which itself stays fixed
 * while the ground rotates under it — see sky_frame_ref's doc comment in
 * environment.tsx) and environment.tsx separately re-rotates its own copy
 * for the sky dome shader by the current map azimuth. Don't fold that
 * rotation in here — there's exactly one place that already does it.
 */
function to_local_direction(alt_deg: f64, az_deg: f64, distance: f64): V3 {
  const alt = alt_deg * DEG2RAD;
  const az = az_deg * DEG2RAD;
  const horizontal = Math.cos(alt) * distance;
  // compass bearing -> Cartesian: East (X) = sin(az), North (Y) = cos(az).
  return [Math.sin(az) * horizontal, Math.cos(az) * horizontal, Math.sin(alt) * distance];
}

const SUN_DISTANCE: f64 = 100;
const MOON_DISTANCE: f64 = 100;

export function get_sun_position(lat: f64, lon: f64, now: Date = new Date()): V3 {
  const d = days_since_j2000(now);
  const eq = sun_equatorial(d);
  const { alt, az } = to_horizontal(eq, lat, lon, d);
  return to_local_direction(alt, az, SUN_DISTANCE);
}

export function get_moon_position(lat: f64, lon: f64, now: Date = new Date()): V3 {
  const d = days_since_j2000(now);
  const sun_eq = sun_equatorial(d);
  const eq = moon_equatorial(d, sun_eq.obliquity);
  const { alt, az } = to_horizontal(eq, lat, lon, d);
  return to_local_direction(alt, az, MOON_DISTANCE);
}

/**
 * Illuminated fraction (0=new moon, 1=full moon), from the real angular
 * separation (elongation) between the sun and moon as seen from Earth —
 * `illuminated_fraction = (1 - cos(elongation)) / 2`, the standard
 * relation (elongation 0 = moon right next to the sun = new; elongation
 * 180 = moon opposite the sun = full). Independent of observer lat/lon
 * (elongation is a geocentric quantity), so this only needs the time.
 */
export function get_moon_illuminated_fraction(now: Date = new Date()): f64 {
  const d = days_since_j2000(now);
  const sun_eq = sun_equatorial(d);
  const moon_eq = moon_equatorial(d, sun_eq.obliquity);

  const sr = sun_eq.ra * DEG2RAD, sd = sun_eq.dec * DEG2RAD;
  const mr = moon_eq.ra * DEG2RAD, md = moon_eq.dec * DEG2RAD;
  const cos_elongation = Math.sin(sd) * Math.sin(md) + Math.cos(sd) * Math.cos(md) * Math.cos(sr - mr);
  return (1 - Math.max(-1, Math.min(1, cos_elongation))) / 2;
}

/**
 * Real sunrise/sunset, as fractional local-clock hours (e.g. 6.25 = 6:15am),
 * for the given date/location — theme.ts's day/night theme boundary uses
 * this instead of a fixed 05:00/21:00, so "night" actually starts at dusk
 * wherever the map happens to be centered instead of a constant clock time.
 *
 * Found by sampling get_sun_position's own altitude across the day (every 5
 * minutes) and linearly interpolating the zero-crossings, rather than a
 * separate closed-form sunrise equation — reuses the one already-verified
 * sun-position code path instead of adding a second formula that could
 * disagree with it. Cheap: this only runs once a minute (theme_clock.ts),
 * and it's ~300 iterations of plain trig.
 *
 * Falls back to a plain 06:00/18:00 split if the sun never crosses the
 * horizon that day (polar day/night) — not a real case for this app's
 * expected usage, just a sane default instead of leaving stale values.
 */
export function get_sunrise_sunset(lat: f64, lon: f64, date: Date): { sunrise_hour: f64; sunset_hour: f64 } {
  const SAMPLES_PER_DAY = 288; // every 5 minutes
  const day_start = new Date(date);
  day_start.setHours(0, 0, 0, 0);

  const altitude_at_hour = (hour: f64): f64 => {
    const t = new Date(day_start.getTime() + hour * 3600_000);
    const d = days_since_j2000(t);
    return to_horizontal(sun_equatorial(d), lat, lon, d).alt;
  };

  let sunrise_hour = 6;
  let sunset_hour = 18;
  let found_rise = false;
  let found_set = false;
  let prev_alt = altitude_at_hour(0);
  const step = 24 / SAMPLES_PER_DAY;
  for (let i = 1; i <= SAMPLES_PER_DAY; i++) {
    const hour = i * step;
    const alt = altitude_at_hour(hour);
    if (!found_rise && prev_alt < 0 && alt >= 0) {
      const t = -prev_alt / (alt - prev_alt);
      sunrise_hour = hour - step * (1 - t);
      found_rise = true;
    }
    if (!found_set && found_rise && prev_alt >= 0 && alt < 0) {
      const t = prev_alt / (prev_alt - alt);
      sunset_hour = hour - step * (1 - t);
      found_set = true;
    }
    prev_alt = alt;
  }
  return { sunrise_hour, sunset_hour };
}
