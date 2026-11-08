// Constants every shader shares. The luminance weights are Rec. 709's, as on the CPU.

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;
const INV_4PI: f32 = 0.0795774715;
const LUMA: vec3f = vec3f(0.2126, 0.7152, 0.0722);

// Every count the GPU writes that sizes a dispatch or a draw, and every list length a pass
// bounds its threads by, is read through these two functions. A count past its list's room, or
// a garbage one, then sizes no more than the list holds, and no dispatch past what every device
// takes.

/// The count `cnt` of a list that holds `cap`, held to the list's room.
fn in_cap(cnt: u32, cap: u32) -> u32 {
  return min(cnt, cap);
}

/// How many workgroups of `per` threads take `cnt` items of a list that holds `cap`, a thread
/// an item, held to the workgroups a dispatch takes in one dimension (`WORKGROUPS_CAP`).
fn workgroups(cnt: u32, cap: u32, per: u32) -> u32 {
  return min((in_cap(cnt, cap) + per - 1u) / per, WORKGROUPS_CAP);
}
