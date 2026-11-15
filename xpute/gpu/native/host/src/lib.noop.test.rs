// xpute-gpu-native-host/lib.noop.test.rs

//! These are tests of the host's own bookkeeping, on wgpu's noop backend.
//! That backend makes and maps buffers but runs nothing, and the queue's work
//! is done as it is submitted. No GPU is touched, so these tests need none;
//! lib.test.rs holds those that do.

use xpute_link::mem;

use super::belt::Belt;
use super::object::Object;
use super::*;

const CHUNK_SZ: u64 = 64;

fn host(
    adapter: wgpu::Adapter,
    device: &wgpu::Device,
    queue: wgpu::Queue,
    frame_latency: u32,
) -> GpuHost {
    let cfg = Config {
        preferred_format: wgpu::TextureFormat::Bgra8Unorm,
        configure_at: ConfigureAt::Here,
        frame_latency,
        upload_chunk_sz: CHUNK_SZ,
        slow_ms: None,
        present_mode: wgpu::PresentMode::Fifo,
    };
    GpuHost::new(adapter, device.clone(), queue, cfg, None, || {})
}

fn device() -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::NOOP,
        backend_options: wgpu::BackendOptions {
            noop: wgpu::NoopBackendOptions {
                enable: true,
                ..Default::default()
            },
            ..Default::default()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("the noop adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("the noop device");
    (adapter, device, queue)
}

/// Once every chunk is taken, no more is taken until the queue gives one
/// back, and the pool keeps its size.
#[test]
fn pool_never_grows() {
    let (_, device, queue) = device();
    let mut belt = Belt::new(&device, CHUNK_SZ, 3);
    for _ in 0..3 {
        let (_, at, n) = belt.take(CHUNK_SZ).expect("a free chunk");
        assert_eq!((at, n), (0, CHUNK_SZ));
    }
    assert!(belt.take(4).is_none(), "every chunk taken");
    belt.finish();
    queue.submit([]);
    belt.recall();
    assert!(belt.is_returning());
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    assert!(
        !belt.is_returning(),
        "mapped again once the queue's work is done"
    );
    for _ in 0..3 {
        assert!(belt.take(CHUNK_SZ).is_some(), "a chunk back");
    }
    assert!(belt.take(4).is_none(), "and no more than the pool");
}

/// A chunk is written piece by piece, each piece at the mapping's alignment,
/// and a write past the chunk's room takes what is left.
#[test]
fn take_keeps_alignment() {
    let (_, device, _) = device();
    let mut belt = Belt::new(&device, CHUNK_SZ, 1);
    assert_eq!(belt.take(4).map(|(_, at, n)| (at, n)), Some((0, 4)));
    assert_eq!(belt.take(100).map(|(_, at, n)| (at, n)), Some((8, 56)));
    assert!(belt.take(4).is_none(), "the one chunk full");
}

/// A write larger than the whole pool is staged across it while the host
/// waits for chunks to come back, and the pool keeps its size.
#[test]
fn write_waits_for_chunks() {
    let (adapter, device, queue) = device();
    let mut h = host(adapter, &device, queue, 1);
    let size = 5 * CHUNK_SZ;
    let target = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    h.put(1, Object::Buffer(target));
    h.write(1, 0, &vec![7; size as usize]);
    assert!(
        h.uploads.is_some(),
        "the last pieces wait for the next submit"
    );
    h.submit(core::iter::empty());
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    for _ in 0..2 {
        assert!(
            h.belt.take(CHUNK_SZ).is_some(),
            "a chunk a frame and one more"
        );
    }
    assert!(h.belt.take(4).is_none(), "and no chunk made past them");
}

/// A buffer written and destroyed in one stream is written first.
#[test]
fn write_lands_before_destroy() {
    let (adapter, device, queue) = device();
    let mut h = host(adapter, &device, queue, 1);
    let target = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    h.put(1, Object::Buffer(target));
    h.write(1, 0, &[7; 16]);
    h.destroy(1);
    assert!(h.uploads.is_none(), "the write went to the queue first");
    h.submit(core::iter::empty());
}

/// A host writes the frames it admitted and has not seen done into the
/// device's block, in the word before `STATUS`, at each poll. It admits
/// by the same rule that a page's guest applies to that word.
#[test]
fn native_tells_unfinished() {
    let (adapter, device, queue) = device();
    let mut h = host(adapter, &device, queue, 1);
    let block = super::test::SPARE_AT;
    let unfinished = block + 4 * xpute_gpu_link::spec::device::UNFINISHED_AT as u32;
    h.block = Some(block);
    assert!(h.admit_frame());
    h.count_frame();
    assert!(!h.admit_frame(), "its one frame unfinished");
    h.tell_counts();
    assert_eq!(mem::load_acquire(unfinished), 1);
    h.poll();
    assert_eq!(
        mem::load_acquire(unfinished),
        0,
        "the frame done as the device is polled"
    );
    assert!(h.admit_frame());
}

/// A device lost stays lost, so its owner lets the host go. The first poll
/// after the loss writes `LOST` into the device's block and lets the block go,
/// so a later poll writes nothing over the block the next device's host writes.
#[test]
fn lost_told_once() {
    let (adapter, device, queue) = device();
    let mut h = host(adapter, &device, queue, 1);
    // A block of its own, past the other test's.
    let block = super::test::SPARE_AT + 4 * xpute_gpu_link::spec::device::WORDS as u32;
    let status = block + 4 * xpute_gpu_link::spec::device::STATUS_AT as u32;
    h.block = Some(block);
    assert!(!h.lost());
    h.device_lost.store(true, Ordering::Release);
    assert!(!h.poll(), "a lost device answers nothing");
    assert_eq!(mem::load_acquire(status), status::LOST);
    assert!(h.lost(), "a loss is never cleared");
    mem::store_release(status, status::READY);
    h.poll();
    assert_eq!(
        mem::load_acquire(status),
        status::READY,
        "the block is the next host's"
    );
}
