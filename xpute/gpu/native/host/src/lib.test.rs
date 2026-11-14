// xpute-gpu-native-host/lib.test.rs

use xpute_golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../../link/spec/golden/program.tsv");

/// The guest's memory for this process's tests, set once (xpute-link
/// `mem::set`): the golden records' offsets from its start, and from
/// `SPARE_AT` on, the room the other tests lay their blocks in.
pub(super) const SPARE_AT: u32 = 1 << 24;

pub(super) fn region() {
    static SET: std::sync::Once = std::sync::Once::new();
    SET.call_once(|| {
        let len = 2 * SPARE_AT;
        let base = Vec::leak(vec![0u8; len as usize]).as_mut_ptr();
        xpute_link::mem::set(xpute_link::mem::Region { base, len });
    });
}

fn device() -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device");
    (adapter, device, queue)
}

/// This is a test's config. Its chunk is as large as the largest write a record
/// makes.
fn cfg(frame_latency: u32) -> Config {
    Config {
        preferred_format: wgpu::TextureFormat::Bgra8Unorm,
        configure_at: ConfigureAt::Here,
        frame_latency,
        upload_chunk_sz: 1 << 16,
        slow_ms: None,
        present_mode: wgpu::PresentMode::Fifo,
    }
}

fn host(
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame_latency: u32,
) -> GpuHost {
    GpuHost::new(
        adapter.clone(),
        device.clone(),
        queue.clone(),
        cfg(frame_latency),
        None,
        || {},
    )
}

/// Each host counts its own frames. One at its latency holds back no other
/// host, and its frames end as the device is polled.
#[test]
fn host_paces_frames() {
    let (adapter, device, queue) = device();
    let (busy, other) = (
        host(&adapter, &device, &queue, 1),
        host(&adapter, &device, &queue, 2),
    );
    assert!(busy.admit_frame());
    busy.count_frame();
    assert!(!busy.admit_frame(), "its one frame is unfinished");
    assert!(
        other.admit_frame(),
        "another host's frames are not its own"
    );
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    assert!(busy.admit_frame(), "its frame ended with the queue's work");
}

/// A frame's callback takes it off its count whether the queue calls it or
/// drops it uncalled.
#[test]
fn frame_ends_either_way() {
    let count = Arc::new(AtomicU32::new(0));
    let called = FrameGuard::count(&count);
    let dropped = FrameGuard::count(&count);
    assert_eq!(count.load(Ordering::Relaxed), 2);
    called();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    drop(dropped);
    assert_eq!(count.load(Ordering::Relaxed), 0);
}

/// A device that goes before a frame's work is done drops its callbacks, and
/// the frames they counted are released with them.
#[test]
fn lost_device_frames_released() {
    let (adapter, device, queue) = device();
    let h = host(&adapter, &device, &queue, 1);
    let count = h.unfinished_cnt.clone();
    assert!(h.admit_frame());
    h.count_frame();
    assert_eq!(
        count.load(Ordering::Relaxed),
        1,
        "counted, and never polled"
    );
    device.destroy();
    drop((h, queue, device));
    assert_eq!(
        count.load(Ordering::Relaxed),
        0,
        "released as the device went"
    );
}

/// A surface's handle holds its slot only while it is the one bound there.
/// An old handle's late unbind lets go no newer surface in the slot, and the
/// newer handle unbinds its own, once. A window's surface needs a window, so
/// the test claims the slot as `bind` does, without one.
#[test]
fn stale_handle_inert() {
    let (adapter, device, queue) = device();
    let mut h = host(&adapter, &device, &queue, 2);
    // These are slot 3, generations 1 and 2 (`slot | generation << 16`).
    let (old, newer) = (3 | (1 << 16), 3 | (2 << 16));
    assert!(!h.unbind(old), "nothing is bound");
    assert_eq!(h.claim(old), Some(3));
    assert_eq!(h.claim(newer), Some(3), "the newer takes the slot");
    assert_eq!(
        h.claim(old),
        None,
        "an old handle's late bind takes nothing back"
    );
    assert_eq!(h.claim(newer), Some(3), "the newer binds again");
    assert!(!h.unbind(old), "an old handle unbinds nothing");
    assert_eq!(h.surfaces[3].handle, Some(newer));
    assert!(h.unbind(newer), "the newer unbinds its own");
    assert!(!h.unbind(newer), "and only once");
    assert_eq!(
        h.claim(old),
        None,
        "the slot remembers the newer after it is let go"
    );
    assert!(!h.unbind(0), "0 is never a handle");
}

/// Each program's recorded runs (the golden's `turn` records), run as they
/// were written, read back the bytes the record holds. Run K's ticket is
/// K + 1, and the next run waits until the host has stored it at the fence.
#[test]
fn records_read_back() {
    let g = Golden::load(GOLDEN);
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .expect("a device with timestamp queries");
    let [size, input_at, done, readback] =
        ["region", "input.at", "done.at", "readback.at"].map(|k| g.u32(k));
    assert!(
        size <= SPARE_AT,
        "the golden region fits before the spare room"
    );
    region();
    // SAFETY: the test's own region, which the host touches only inside
    // `run` and `poll`.
    let write = |at, from: &[u8]| unsafe { xpute_link::mem::write(at, from) };
    let word = |at| unsafe { xpute_link::mem::word(at) };
    g.each("program", |name| {
        let p = g.s(name);
        write(0, &vec![0u8; size as usize]);
        let key = format!("{p}.input");
        if g.has(&key) {
            write(input_at, &g.bytes(&key));
        }
        let mut host = GpuHost::new(
            adapter.clone(),
            device.clone(),
            queue.clone(),
            cfg(2),
            None,
            || {},
        );
        // A recorded turn is a stream: its record words' count, its head's
        // length in words, then the records the host is lent.
        g.each(&format!("{p}.turn"), |turn| {
            let words = g.bytes(turn);
            write(0, &words);
            let (end, head) = (word(0), word(4));
            let ticket = turn.rsplit('.').next().unwrap().parse::<u32>().unwrap() + 1;
            host.run(4 * head, 4 * end);
            while xpute_link::mem::load_acquire(done) != ticket {
                let _ = device.poll(wgpu::PollType::wait_indefinitely());
                host.poll();
            }
        });
        let want = g.bytes(&format!("{p}.readback"));
        assert_eq!(
            // SAFETY: as `write`.
            unsafe { xpute_link::mem::read(readback, want.len() as u32) },
            &want[..],
            "{p}.readback"
        );
    });
}
