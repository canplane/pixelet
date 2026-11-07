// Shared constants; luminance weights are Rec. 709, as on the CPU.

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;
const INV_4PI: f32 = 0.0795774715;
const LUMA: vec3f = vec3f(0.2126, 0.7152, 0.0722);

// Every count the GPU writes that sizes a dispatch or a draw, and every list length a pass
// bounds its threads by, is read through these two, so a count past its list's room, or a
// garbage one, sizes no more than the list holds and no dispatch past what every device takes.

/// Count `cnt` of a list that holds `room`, at most its room.
fn in_room(cnt: u32, room: u32) -> u32 {
  return min(cnt, room);
}

/// The workgroups of `per` threads that take `cnt` of a list that holds `room`, a thread each,
/// at most the workgroups a dispatch takes a dimension (`WORKGROUPS_MOST`).
fn workgroups(cnt: u32, room: u32, per: u32) -> u32 {
  return min((in_room(cnt, room) + per - 1u) / per, WORKGROUPS_MOST);
}
