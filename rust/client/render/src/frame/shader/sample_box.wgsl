// pxl-render/sample_box.wgsl

// Each sample of the camera grid, a thread each, written as a box for the raster's draw
// (boxes.wgsl, the eye as sample_eye.wgsl takes it), the one across and up before it each
// sampled; and what lies about its face asked once a box (`ask`), as its pixels draw the
// face (pxl-shader raster.rs `sample_box_cs`). The shadow map's is light_sample_box.wgsl,
// which asks none.

/// What lies about the face sample `s` entered, whose box is at `place` among the
/// boxes: told at once where it needs no asking, a lack none, left in its words
/// (`beam::SAMPLE_FACES`) where its pixels read it; else its ask at `place`
/// (`Grid::ask_at`), off the line its cell's rays resumed from, for the faces' pass,
/// its faces to be left there (pxl-shader raster.rs `ask`).
fn ask_face(s: vec2<u32>, place: u32) {
  let k = sample_word(s);
  let a = eye_word(GRID + GRID_ASKS_AT) + place * ASK_WORDS;
  let lacks = beams[k + SAMPLE_LACKS] != 0u;
  let fill = (beams[k + SAMPLE_ASSET] >> FILL_SHIFT) & (EIGHTHS - 1u);
  if (lacks || fill != 0u) {
    beams[k + SAMPLE_FACES] = select(FACE_BOX, 0u, lacks);
    beams[a + ASK_TO] = ASK_NONE;
    return;
  }
  // The face its ray enters across its axis: 2·axis, plus one where the ray runs up
  // it (pxl-shader world.rs `face_of`).
  let dir = sample_aim(s).dir;
  let axis = beams[k + SAMPLE_AXIS];
  let along = select(select(dir.z, dir.y, axis == 1u), dir.x, axis == 0u);
  beams[a + ASK_NODE] = k + SAMPLE_NODE;
  beams[a + ASK_FACE] = 2u * axis + select(0u, 1u, along > 0.0);
  beams[a + ASK_LINE] = beams[sample_cell_word(s) + CELL_CORNER] * LINE_WORDS;
  beams[a + ASK_TO] = k + SAMPLE_FACES;
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn sample_box_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let origin = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  let place = sample_box(id.xy, origin, true, vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u)));
  if (place != NO_BOX) {
    ask_face(sample_taken(id.xy), place);
    // The faces' pass's workgroups raised to hold it (pxl-shader lib.rs `opens`).
    if (place % RASTER_FACES_LANES == 0u) {
      args_most(RASTER_ARGS_FACES, place / RASTER_FACES_LANES + 1u);
    }
  }
}
