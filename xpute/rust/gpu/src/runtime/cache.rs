// xpute-gpu/runtime/cache.rs

//! This keeps the device's compiled pipelines across launches, where the device
//! can (wgpu's `PIPELINE_CACHE`): a pipeline made through the cache is looked up
//! in what an earlier launch compiled before it is compiled again. The cache's
//! data lives in a file under the directory the platform keeps caches in, named
//! for the device it was compiled for (`pipeline_cache_key`), and wgpu refuses
//! data another device or another wgpu made.

use std::path::{Path, PathBuf};

pub(super) struct Cache {
    pub(super) pipelines: wgpu::PipelineCache,
    file: PathBuf,
}

impl Cache {
    /// The cache of `device` in `dir`, loaded from the file an earlier launch saved
    /// there. None where the device keeps no pipelines.
    pub(super) fn open(
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        dir: &Path,
    ) -> Option<Cache> {
        if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
            return None;
        }
        let file = dir.join(wgpu::util::pipeline_cache_key(&adapter.get_info())?);
        let data = std::fs::read(&file).ok();
        // SAFETY: the data is what `save` wrote, which wgpu checks before it reads,
        // and data it refuses gives an empty cache (`fallback`).
        let pipelines = unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: None,
                data: data.as_deref(),
                fallback: true,
            })
        };
        Some(Cache { pipelines, file })
    }

    /// Writes what the cache holds over its file, through a file beside it, so that
    /// a launch never reads half of one. A cache that cannot be written is compiled
    /// again by the next launch.
    pub(super) fn save(&self) {
        let Some(data) = self.pipelines.get_data() else {
            return;
        };
        let next = self.file.with_extension("next");
        if std::fs::write(&next, &data).is_ok() {
            let _ = std::fs::rename(&next, &self.file);
        }
    }
}
