//! node_seed hand-written in WGSL, three ways of reading a node's i64
//! coordinates, to find which step the generated shader gets wrong on Metal.

const WGSL: &str = r#"
@group(0) @binding(0) var<storage, read_write> out: array<u32>;
fn mix64(x0: u64) -> u64 {
    var z = x0 + 11400714819323198485lu;
    z = (z ^ (z >> 30u)) * 13787848793156543929lu;
    z = (z ^ (z >> 27u)) * 10723151780598845931lu;
    return z ^ (z >> 31u);
}
fn put(i: u32, h: u64) {
    out[2u * i] = u32(h & 0xfffffffflu);
    out[2u * i + 1u] = u32(h >> 32u);
}
@compute @workgroup_size(1)
fn main() {
    let first = vec3<i32>(262140, -4, -4);
    let head = mix64(mix64(15042267673225134081lu ^ 3397255169lu) ^ u64(19u));
    // 0: unrolled, i64 from i32, bitcast to u64.
    var h = head;
    h = mix64(h ^ bitcast<u64>(i64(first.x)));
    h = mix64(h ^ bitcast<u64>(i64(first.y)));
    h = mix64(h ^ bitcast<u64>(i64(first.z)));
    put(0u, h);
    // 1: through a local array<i64, 3>, indexed in a loop, as the generated code does.
    var c = array<i64, 3>(i64(first.x), i64(first.y), i64(first.z));
    var g = head;
    for (var k = 0u; k < 3u; k++) {
        g = mix64(g ^ bitcast<u64>(c[k]));
    }
    put(1u, g);
    // 2: u64(i64) instead of bitcast.
    var u = head;
    u = mix64(u ^ u64(i64(first.x)));
    u = mix64(u ^ u64(i64(first.y)));
    u = mix64(u ^ u64(i64(first.z)));
    put(2u, u);
    // 3: the generated shader's loop, its shape kept: phis, `continuing`,
    // `break if`, and the value read at the loop's top.
    var local_6 = array<i64, 3>(i64(first.x), i64(first.y), i64(first.z));
    var phi_749_: u32;
    var phi_752_: u64;
    var phi_750_: u32;
    var phi_753_: u64;
    var phi_980_: bool;
    var local_7: u64;
    phi_749_ = 0u;
    phi_752_ = head;
    loop {
        let _e128 = phi_749_;
        let _e130 = phi_752_;
        local_7 = _e130;
        let _e131 = (_e128 < 3u);
        if _e131 {
            let _e133 = local_6[_e128];
            let _e136 = ((_e130 ^ bitcast<u64>(_e133)) + 11400714819323198485lu);
            let _e140 = ((_e136 ^ (_e136 >> 30u)) * 13787848793156543929lu);
            let _e144 = ((_e140 ^ (_e140 >> 27u)) * 10723151780598845931lu);
            phi_750_ = (_e128 + 1u);
            phi_753_ = (_e144 ^ (_e144 >> 31u));
        } else {
            phi_750_ = u32();
            phi_753_ = u64();
        }
        let _e150 = phi_750_;
        let _e152 = phi_753_;
        continue;
        continuing {
            phi_749_ = _e150;
            phi_752_ = _e152;
            phi_980_ = false;
            break if !(_e131);
        }
    }
    put(3u, local_7);
}
"#;

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { required_features: wgpu::Features::SHADER_INT64, ..Default::default() })).expect("device");
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(WGSL.into()) });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: None, layout: None, module: &module, entry_point: Some("main"), compilation_options: Default::default(), cache: None });
    let out = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: 32, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let read = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: 32, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &pipeline.get_bind_group_layout(0), entries: &[wgpu::BindGroupEntry { binding: 0, resource: out.as_entire_binding() }] });
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    enc.copy_buffer_to_buffer(&out, 0, &read, 0, 32);
    queue.submit([enc.finish()]);
    read.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let w: Vec<u32> = read.slice(..).get_mapped_range().expect("mapped").chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let want = pxl_spec::procedural::node_seed(0xca7e_0001, (19, [262140i64, -4, -4]));
    for (i, name) in ["unrolled bitcast", "array<i64,3> loop", "u64(i64)", "generated loop shape"].iter().enumerate() {
        let got = u64::from(w[2 * i]) | u64::from(w[2 * i + 1]) << 32;
        println!("{name}: gpu {got:016x} cpu {want:016x} {}", if got == want { "same" } else { "DIFFER" });
    }
}
