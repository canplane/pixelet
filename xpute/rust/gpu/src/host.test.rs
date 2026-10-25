// xpute-gpu/host.test.rs

use xpute_kit::golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../../spec/golden/gpu/programs.tsv");

fn device() -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("an adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device");
    (adapter, device, queue)
}

/// A test's plan: `frame_latency` frames, and a chunk as large as the
/// largest write a record makes.
fn plan(frame_latency: u32) -> Plan {
    Plan {
        preferred: wgpu::TextureFormat::Bgra8Unorm,
        configure_at: ConfigureAt::Here,
        frame_latency,
        upload_chunk_bytes: 1 << 16,
        present_mode: wgpu::PresentMode::Fifo,
    }
}

fn host(adapter: &wgpu::Adapter, device: &wgpu::Device, queue: &wgpu::Queue, frame_latency: u32) -> GpuHost {
    GpuHost::new(adapter.clone(), device.clone(), queue.clone(), plan(frame_latency), || {})
}

/// Each host counts its own frames: one at its latency holds back no other,
/// and its frames end as the device is polled.
#[test]
fn each_host_paces_its_own_frames() {
    let (adapter, device, queue) = device();
    let (mut busy, mut other) = (host(&adapter, &device, &queue, 1), host(&adapter, &device, &queue, 2));
    assert!(busy.admit());
    busy.present();
    assert!(!busy.admit(), "its one frame is unfinished");
    assert!(other.admit(), "another host's frames are not its own");
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    assert!(busy.admit(), "its frame ended with the queue's work");
}

/// A frame's callback takes it off its count whether the queue calls it or
/// lets it go uncalled.
#[test]
fn a_frame_ends_whether_its_callback_is_called_or_let_go() {
    let count = Arc::new(AtomicU32::new(0));
    let called = Unfinished::count(&count);
    let dropped = Unfinished::count(&count);
    assert_eq!(count.load(Ordering::Relaxed), 2);
    called();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    drop(dropped);
    assert_eq!(count.load(Ordering::Relaxed), 0);
}

/// A device that goes with a frame's work never done lets its callbacks go,
/// and the frames they counted are released with them.
#[test]
fn a_gone_devices_frames_are_released() {
    let (adapter, device, queue) = device();
    let mut h = host(&adapter, &device, &queue, 1);
    let count = h.unfinished.clone();
    assert!(h.admit());
    h.present();
    assert_eq!(count.load(Ordering::Relaxed), 1, "counted, and never polled");
    device.destroy();
    drop((h, queue, device));
    assert_eq!(count.load(Ordering::Relaxed), 0, "released as the device went");
}

/// A surface's handle holds its slot only while it is the one bound there:
/// an old handle's late unbind lets go no newer surface in the slot, and the
/// newer one unbinds its own, once. A window's surface needs a window, so
/// the slot is claimed as `bind` claims it, without one.
#[test]
fn an_old_handle_binds_and_unbinds_no_newer_surface() {
    let (adapter, device, queue) = device();
    let mut h = host(&adapter, &device, &queue, 2);
    // Slot 3, generations 1 and 2 (`slot | generation << 16`).
    let (old, newer) = (3 | (1 << 16), 3 | (2 << 16));
    assert!(!h.unbind(old), "nothing is bound");
    assert_eq!(h.claim(old), Some(3));
    assert_eq!(h.claim(newer), Some(3), "the newer takes the slot");
    assert_eq!(h.claim(old), None, "an old handle's late bind takes nothing back");
    assert_eq!(h.claim(newer), Some(3), "the newer binds again");
    assert!(!h.unbind(old), "an old handle unbinds nothing");
    assert_eq!(h.surfaces[3].handle, newer);
    assert!(h.unbind(newer), "the newer unbinds its own");
    assert!(!h.unbind(newer), "and only once");
    assert_eq!(h.claim(old), None, "the slot remembers the newer after it is let go");
    assert!(!h.unbind(0), "0 is never a handle");
}

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
        let mut host = GpuHost::new(adapter.clone(), device.clone(), queue.clone(), plan(2), || {});
        g.each(&format!("{p}.turn"), |turn| {
            let words = g.bytes(turn);
            memory[..words.len()].copy_from_slice(&words);
            let ticket = turn.rsplit('.').next().unwrap().parse::<u32>().unwrap() + 1;
            Memory::of(&mut memory).set_word(done, ticket);
            host.run(&mut Memory::of(&mut memory), 0, head);
            while Memory::of(&mut memory).word(done) & DONE == 0 {
                let _ = device.poll(wgpu::PollType::wait_indefinitely());
                host.poll(&mut Memory::of(&mut memory));
            }
        });
        let want = g.bytes(&format!("{p}.readback"));
        assert_eq!(&memory[readback as usize..readback as usize + want.len()], &want[..], "{p}.readback");
    });
}
