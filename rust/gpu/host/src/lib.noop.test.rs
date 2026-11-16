// pxl-gpu-host/lib.noop.test.rs

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

fn belt(device: &wgpu::Device, chunk_cnt: u32) -> Belt {
    Belt::new(device, CHUNK_SZ, chunk_cnt)
}

/// Every chunk taken, the pool keeps its size: the chunks come back free once
/// the queue's work is done, and a submit begins the next room at a fresh
/// chunk.
#[test]
fn pool_never_grows() {
    let (_, device, queue) = device();
    let mut belt = belt(&device, 3);
    for _ in 0..3 {
        let (_, at, n) = belt.take(CHUNK_SZ, 1).expect("a free chunk");
        assert_eq!((at, n), (0, CHUNK_SZ));
    }
    assert_eq!(belt.free_cnt(), 0, "every chunk taken");
    belt.finish();
    queue.submit([]);
    belt.recall();
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(
        belt.free_cnt(),
        3,
        "mapped again once the queue's work is done"
    );
    assert_eq!(belt.take(4, 1).map(|(_, at, n)| (at, n)), Some((0, 4)));
    assert_eq!(belt.free_cnt(), 2);
}

/// A run that finds no free chunk, as a write that cannot wait may, makes one
/// more, which is let go once it is back, so the pool keeps its size.
#[test]
fn take_past_pool_makes_one_more() {
    let (_, device, queue) = device();
    let mut belt = belt(&device, 1);
    assert!(belt.take(CHUNK_SZ, 1).is_some());
    assert_eq!(
        belt.take(4, 1).map(|(_, at, n)| (at, n)),
        Some((0, 4)),
        "a chunk made for the run"
    );
    belt.finish();
    queue.submit([]);
    belt.recall();
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(belt.free_cnt(), 1, "the pool's one chunk back");
    assert!(belt.take(4, 1).is_some(), "and the one made let go");
    assert_eq!(belt.free_cnt(), 0);
}

/// A chunk is written piece by piece, each piece at the room's alignment,
/// and a write past the chunk's room takes what is left, then goes on in the
/// next chunk.
#[test]
fn take_keeps_alignment() {
    let (_, device, _) = device();
    let mut belt = belt(&device, 2);
    assert_eq!(belt.take(4, 1).map(|(_, at, n)| (at, n)), Some((0, 4)));
    assert_eq!(belt.take(100, 1).map(|(_, at, n)| (at, n)), Some((16, 48)));
    assert_eq!(belt.take(52, 1).map(|(_, at, n)| (at, n)), Some((0, 52)));
}

/// A texture's write takes whole rows: the rows the chunk has left, and a
/// fresh chunk where it has less than one.
#[test]
fn take_keeps_rows() {
    let (_, device, _) = device();
    let mut belt = belt(&device, 2);
    assert_eq!(belt.take(4, 1).map(|(_, at, n)| (at, n)), Some((0, 4)));
    assert_eq!(belt.take(60, 20).map(|(_, at, n)| (at, n)), Some((16, 40)));
    assert_eq!(belt.take(20, 20).map(|(_, at, n)| (at, n)), Some((0, 20)));
}

/// A chunk whose mapping is refused holds the device lost.
#[test]
fn refused_mapping_is_lost() {
    let (adapter, device, queue) = device();
    let mut h = host(adapter, &device, queue, 1);
    h.belt.take(4, 1).expect("a free chunk");
    h.belt.finish();
    device.destroy();
    h.belt.recall();
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    assert!(h.lost(), "the remap refused");
    assert!(h.belt.take(4, 1).is_none(), "and no room taken after");
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

/// A host writes the frames it counted and has not seen done into the
/// device's block, in the word before `STATUS`, at each poll.
#[test]
fn native_tells_unfinished() {
    let (adapter, device, queue) = device();
    let mut h = host(adapter, &device, queue, 1);
    let block = test::mem::alloc(4 * device::WORDS as u32);
    let unfinished = block + 4 * device::UNFINISHED_AT as u64;
    h.block = Some(block);
    h.count_frame();
    h.tell_counts();
    assert_eq!(mem::load_acquire(unfinished), 1);
    h.poll();
    assert_eq!(
        mem::load_acquire(unfinished),
        0,
        "the frame done as the device is polled"
    );
}

/// A host writes its belt's free chunks into the device's block, and says so
/// where more are free than it last wrote, which wakes a guest whose writes
/// wait for room.
#[test]
fn native_tells_freed() {
    let (adapter, device, queue) = device();
    let mut h = host(adapter, &device, queue, 1);
    let block = test::mem::alloc(4 * device::WORDS as u32);
    let free = block + 4 * device::FREE_CHUNKS_AT as u64;
    h.block = Some(block);
    assert!(h.tell_counts(), "none written before, two free");
    for _ in 0..2 {
        h.belt.take(CHUNK_SZ, 1).expect("a free chunk");
    }
    assert!(!h.tell_counts(), "every chunk taken");
    assert_eq!(mem::load_acquire(free), 0);
    h.uploads = Some(device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default()));
    h.submit(core::iter::empty());
    assert!(!h.tell_counts(), "on their way back");
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    assert!(h.tell_counts(), "back");
    assert_eq!(mem::load_acquire(free), 2);
    assert!(!h.tell_counts(), "and said once");
}

/// A device lost stays lost, so its owner lets the host go. The first poll
/// after the loss writes `LOST` into the device's block and lets the block go,
/// so a later poll writes nothing over the block the next device's host writes.
#[test]
fn lost_told_once() {
    let (adapter, device, queue) = device();
    let mut h = host(adapter, &device, queue, 1);
    let block = test::mem::alloc(4 * device::WORDS as u32);
    let status = block + 4 * device::STATUS_AT as u64;
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
