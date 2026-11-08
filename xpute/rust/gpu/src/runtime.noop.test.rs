// xpute-gpu/runtime.noop.test.rs

//! These are tests of the runtime's own bookkeeping, on wgpu's noop backend.
//! That backend makes and maps buffers but runs nothing, and the queue's work
//! is done as it is submitted. No GPU is touched, so these tests need none;
//! runtime.test.rs holds those that do.

use xpute_runtime::mem::Memory;

use super::belt::Belt;
use super::object::Object;
use super::*;

const CHUNK_BYTES: u64 = 64;

fn runtime(
    adapter: wgpu::Adapter,
    device: &wgpu::Device,
    queue: wgpu::Queue,
    frame_latency: u32,
) -> GpuRuntime {
    let cfg = Config {
        preferred_format: wgpu::TextureFormat::Bgra8Unorm,
        configure_at: ConfigureAt::Here,
        frame_latency,
        upload_chunk_bytes: CHUNK_BYTES,
        present_mode: wgpu::PresentMode::Fifo,
    };
    GpuRuntime::new(adapter, device.clone(), queue, cfg, || {})
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
    let mut belt = Belt::new(&device, CHUNK_BYTES, 3);
    for _ in 0..3 {
        let (_, at, n) = belt.take(CHUNK_BYTES).expect("a free chunk");
        assert_eq!((at, n), (0, CHUNK_BYTES));
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
        assert!(belt.take(CHUNK_BYTES).is_some(), "a chunk back");
    }
    assert!(belt.take(4).is_none(), "and no more than the pool");
}

/// A chunk is written piece by piece, each piece at the mapping's alignment,
/// and a write past the chunk's room takes what is left.
#[test]
fn take_keeps_alignment() {
    let (_, device, _) = device();
    let mut belt = Belt::new(&device, CHUNK_BYTES, 1);
    assert_eq!(belt.take(4).map(|(_, at, n)| (at, n)), Some((0, 4)));
    assert_eq!(belt.take(100).map(|(_, at, n)| (at, n)), Some((8, 56)));
    assert!(belt.take(4).is_none(), "the one chunk full");
}

/// A write larger than the whole pool is staged across it while the runtime
/// waits for chunks to come back, and the pool keeps its size.
#[test]
fn write_waits_for_chunks() {
    let (adapter, device, queue) = device();
    let mut h = runtime(adapter, &device, queue, 1);
    let size = 5 * CHUNK_BYTES;
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
            h.belt.take(CHUNK_BYTES).is_some(),
            "a chunk a frame and one more"
        );
    }
    assert!(h.belt.take(4).is_none(), "and no chunk made past them");
}

/// A runtime writes the frames it admitted and has not seen done into the
/// device's block, in the word before STATUS, as each turn begins. It admits
/// by the same rule that a page's core applies to that word.
#[test]
fn native_tells_unfinished() {
    let (adapter, device, queue) = device();
    let mut h = runtime(adapter, &device, queue, 1);
    let mut bytes = [0u8; 4 * crate::device::WORDS];
    let mut memory = Memory::of(&mut bytes);
    let unfinished = 4 * crate::device::UNFINISHED_AT as u32;
    h.block = Some(0);
    assert!(h.admit_frame());
    h.present();
    assert!(!h.admit_frame(), "its one frame unfinished");
    h.tell_unfinished(&mut memory);
    assert_eq!(memory.load_acquire(unfinished), 1);
    h.poll(&mut memory);
    assert_eq!(
        memory.load_acquire(unfinished),
        0,
        "the frame done as the device is polled"
    );
    assert!(h.admit_frame());
}
