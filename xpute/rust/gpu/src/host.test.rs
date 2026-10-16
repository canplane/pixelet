// xpute-gpu/host.test.rs

use xpute_kit::golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../../spec/golden/gpu/programs.tsv");

/// Each program's recorded turns, run as they were written, read back the
/// bytes the record holds: turn K's ticket, K + 1, stands at the done word
/// as the guest left it, and the next turn waits for its DONE.
#[test]
fn every_record_reads_back_its_bytes_on_a_device() {
    let g = Golden::load(GOLDEN);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("an adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .expect("a device with timestamp queries");
    let [region, head, input_at, done, readback] = ["region", "stream.head", "input.at", "done.at", "readback.at"].map(|k| g.u32(k));
    g.each("program", |name| {
        let p = g.s(name);
        let mut memory = vec![0u8; region as usize];
        let key = format!("{p}.input");
        if g.has(&key) {
            let input = g.bytes(&key);
            memory[input_at as usize..input_at as usize + input.len()].copy_from_slice(&input);
        }
        let mut host = GpuHost::new(adapter.clone(), device.clone(), queue.clone());
        g.each(&format!("{p}.turn"), |turn| {
            let words = g.bytes(turn);
            memory[..words.len()].copy_from_slice(&words);
            let ticket = turn.rsplit('.').next().unwrap().parse::<u32>().unwrap() + 1;
            set_word(&mut memory, done, ticket);
            host.run(&mut memory, 0, head);
            while word_at(&memory, done) & DONE == 0 {
                let _ = device.poll(wgpu::PollType::wait_indefinitely());
                host.poll(&mut memory);
            }
        });
        let want = g.bytes(&format!("{p}.readback"));
        assert_eq!(&memory[readback as usize..readback as usize + want.len()], &want[..], "{p}.readback");
    });
}
