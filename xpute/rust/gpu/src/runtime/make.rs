// xpute-gpu/runtime/make.rs

//! This is WebGPU's async pipeline call on a native device. A browser compiles
//! such a pipeline apart from the page; here a thread of the runtime's own
//! compiles it apart from the stream, and a later poll puts it at its handle
//! and answers its fence.

use xpute_core::abi::handle::handle_slot;
use xpute_runtime::mem::Memory;

use super::object::Object;
use super::pending::signal;
use super::word::WordCursor;
use super::worker::Worker;
use super::GpuRuntime;

/// This is how to make a pipeline, with everything it needs.
pub(super) type Make = Box<dyn FnOnce() -> Object + Send>;

pub(super) type Maker = Worker<(u32, Make), (u32, Object)>;

pub(super) fn maker(wake: fn()) -> Maker {
    Worker::new(wake, |(id, make): (u32, Make)| (id, make()))
}

/// A `Making` is a pipeline sent to the maker, kept at its handle's slot. A
/// handle destroyed or made anew meanwhile ends it, and its fence is not
/// answered.
pub(super) struct Making {
    id: u32,
    word: u32,
    ticket: u32,
}

impl GpuRuntime {
    /// Sends the pipeline to the maker. Its fence's words follow its record.
    pub(super) fn make_later(&mut self, (id, make): (u32, Make), r: &mut WordCursor) {
        let (word, ticket) = (r.u(), r.u());
        self.making
            .insert(handle_slot(id), Making { id, word, ticket });
        let _ = self.maker.ask.send((id, make));
    }

    /// Puts each pipeline made since the last poll at its handle and answers
    /// its fence.
    pub(super) fn land_pipelines(&mut self, mem: &mut Memory<'_>) {
        while let Ok((id, pipeline)) = self.maker.answers.try_recv() {
            let slot = handle_slot(id);
            let Some(&Making { word, ticket, .. }) = self.making.get(&slot).filter(|m| m.id == id)
            else {
                continue;
            };
            self.making.remove(&slot);
            self.put(id, pipeline);
            signal(mem, word, ticket, None);
        }
    }
}
