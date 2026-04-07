// @pxl/web/render/primitives/sphere/mesh.ts

type SphereMesh = {
  vertices: Float32Array;
  indices: Uint16Array;
};

function build_sphere_mesh(lat_steps: number, lon_steps: number): SphereMesh {
  const vertices: number[] = [];
  const indices: number[] = [];

  for (let iy = 0; iy <= lat_steps; iy++) {
    const v = iy / lat_steps;
    const phi = v * Math.PI;
    const sin_phi = Math.sin(phi);
    const cos_phi = Math.cos(phi);

    for (let ix = 0; ix <= lon_steps; ix++) {
      const u = ix / lon_steps;
      const theta = u * Math.PI * 2;
      const sin_theta = Math.sin(theta);
      const cos_theta = Math.cos(theta);

      vertices.push(
        sin_phi * cos_theta,
        sin_phi * sin_theta,
        cos_phi,
      );
    }
  }

  const row = lon_steps + 1;

  for (let iy = 0; iy < lat_steps; iy++) {
    for (let ix = 0; ix < lon_steps; ix++) {
      const a = iy * row + ix;
      const b = a + 1;
      const c = a + row;
      const d = c + 1;

      if (iy !== 0) {
        indices.push(a, c, b);
      }

      if (iy !== lat_steps - 1) {
        indices.push(b, c, d);
      }
    }
  }

  return {
    vertices: new Float32Array(vertices),
    indices: new Uint16Array(indices),
  };
}

const mesh = build_sphere_mesh(10, 16);

export const SPHERE_VERTICES = mesh.vertices;
export const SPHERE_INDICES = mesh.indices;
