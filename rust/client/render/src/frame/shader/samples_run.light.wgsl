// pxl-render/samples_run.light.wgsl

// The light's samples' run as the queue's passes count its work (queue_walk.wgsl,
// queue_ask.wgsl, queue_last.wgsl; samples.wgsl): its walks' tag in the node cache's keys, the
// light's lattice's caps, and the lanes its visits and asks are counted in.

const WALK_PASS: u32 = LIGHT_SAMPLES_PASS;

/// The first of the light's lattice's caps in the node cache's head (pxl-engine
/// `node_cache::caps_at`, `count::LIGHT_LATTICE`).
fn queue_caps() -> u32 {
  return LIGHT_LATTICE_CAPS;
}

/// Where the asks' records lie in its queue's view (pxl-engine `ray::queue_records_at`,
/// `ray::QUEUE_LIGHT_SAMPLES`).
fn queue_records_at() -> u32 {
  return LIGHT_SAMPLES_RECORDS_AT;
}

fn visits_at() -> u32 {
  return LIGHT_SAMPLE_COUNT;
}

fn asked_at() -> u32 {
  return LIGHT_ASKED_COUNT;
}

/// A sample its run leaves parked owes nothing: every slot of its tile is answered by the
/// voxel the light's raster keeps there, by the sky or by its own walk (light.wgsl), the
/// samples only finding what the raster draws.
fn sample_owed() {
}
