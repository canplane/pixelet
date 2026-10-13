//! mix64 hand-written in WGSL, no rust-gpu: whether Metal's u64 through
//! wgpu 30 / Naga 30 gives xpute-kit's bits.

const WGSL: &str = r#"
@group(0) @binding(0) var<storage, read_write> out: array<u32>;
fn mix64(x0: u64) -> u64 {
    var z = x0 + 11400714819323198485lu;
    z = (z ^ (z >> 30u)) * 13787848793156543929lu;
    z = (z ^ (z >> 27u)) * 10723151780598845931lu;
    return z ^ (z >> 31u);
}
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = u64(id.x) * 0x9e3779b97f4a7c15lu;
    let h = mix64(x);
    out[2u * id.x] = u32(h & 0xfffffffflu);
    out[2u * id.x + 1u] = u32(h >> 32u);
}
"#;

fn mix64(x: u64) -> u64 {
    let x = x.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { required_features: wgpu::Features::SHADER_INT64, ..Default::default() })).expect("device");
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(WGSL.into()) });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: None, layout: None, module: &module, entry_point: Some("main"), compilation_options: Default::default(), cache: None });
    let n = 64u32;
    let out = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: u64::from(n) * 8, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let read = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: u64::from(n) * 8, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &pipeline.get_bind_group_layout(0), entries: &[wgpu::BindGroupEntry { binding: 0, resource: out.as_entire_binding() }] });
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(n, 1, 1);
    }
    enc.copy_buffer_to_buffer(&out, 0, &read, 0, u64::from(n) * 8);
    queue.submit([enc.finish()]);
    read.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let w: Vec<u32> = read.slice(..).get_mapped_range().expect("mapped").chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let mut differ = 0;
    for i in 0..n as usize {
        let got = u64::from(w[2 * i]) | u64::from(w[2 * i + 1]) << 32;
        let want = mix64((i as u64).wrapping_mul(0x9e3779b97f4a7c15));
        if got != want {
            differ += 1;
            if differ <= 3 {
                println!("  mix64 #{i}: gpu {got:016x} cpu {want:016x}");
            }
        }
    }
    println!("hand WGSL mix64 on Metal: {n} values, {differ} differ");
}
