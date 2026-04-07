// @pxl/web/render/common/gpu_buffer.ts

export function create_static_buffer(
  device: GPUDevice,
  usage: GPUBufferUsageFlags,
  data: Float32Array | Uint16Array,
): GPUBuffer {
  const buf = device.createBuffer({
    size: data.byteLength,
    usage,
    mappedAtCreation: true,
  });

  if (data instanceof Float32Array) {
    new Float32Array(buf.getMappedRange()).set(data);
  } else {
    new Uint16Array(buf.getMappedRange()).set(data);
  }

  buf.unmap();
  return buf;
}
