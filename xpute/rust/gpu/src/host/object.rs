// xpute-gpu/host/object.rs

//! The objects a guest made, each at its handle's slot.

use xpute_guest::abi::handle::handle_slot;

use super::GpuHost;

pub(super) enum Object {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
    /// With its format, which a depth attachment asks of it.
    View(wgpu::TextureView, wgpu::TextureFormat),
    Sampler(wgpu::Sampler),
    Shader(wgpu::ShaderModule),
    BindGroupLayout(wgpu::BindGroupLayout),
    PipelineLayout(wgpu::PipelineLayout),
    BindGroup(wgpu::BindGroup),
    RenderPipeline(wgpu::RenderPipeline),
    ComputePipeline(wgpu::ComputePipeline),
    QuerySet(wgpu::QuerySet),
    Encoder(wgpu::CommandEncoder),
    /// Boxed, so a slot is a handle's size and not a pass's.
    RenderPass(Box<wgpu::RenderPass<'static>>),
    ComputePass(Box<wgpu::ComputePass<'static>>),
}

impl GpuHost {
    pub(super) fn put(&mut self, id: u32, o: Object) {
        let slot = handle_slot(id) as usize;
        if self.objects.len() <= slot {
            self.objects.resize_with(slot + 1, || None);
        }
        self.objects[slot] = Some(o);
    }

    pub(super) fn get(&self, id: u32) -> &Object {
        self.objects.get(handle_slot(id) as usize).and_then(Option::as_ref).unwrap_or_else(|| panic!("[gpu] no object {id}"))
    }

    pub(super) fn get_mut(&mut self, id: u32) -> &mut Object {
        self.objects
            .get_mut(handle_slot(id) as usize)
            .and_then(Option::as_mut)
            .unwrap_or_else(|| panic!("[gpu] no object {id}"))
    }

    pub(super) fn take(&mut self, id: u32) -> Option<Object> {
        self.objects.get_mut(handle_slot(id) as usize).and_then(Option::take)
    }

    pub(super) fn buffer(&self, id: u32) -> &wgpu::Buffer {
        match self.get(id) {
            Object::Buffer(b) => b,
            _ => panic!("[gpu] {id} is no buffer"),
        }
    }

    pub(super) fn texture(&self, id: u32) -> &wgpu::Texture {
        match self.get(id) {
            Object::Texture(t) => t,
            _ => panic!("[gpu] {id} is no texture"),
        }
    }

    pub(super) fn view(&self, id: u32) -> (&wgpu::TextureView, wgpu::TextureFormat) {
        match self.get(id) {
            Object::View(v, format) => (v, *format),
            _ => panic!("[gpu] {id} is no view"),
        }
    }

    pub(super) fn query_set(&self, id: u32) -> &wgpu::QuerySet {
        match self.get(id) {
            Object::QuerySet(q) => q,
            _ => panic!("[gpu] {id} is no query set"),
        }
    }

    pub(super) fn shader(&self, id: u32) -> &wgpu::ShaderModule {
        match self.get(id) {
            Object::Shader(s) => s,
            _ => panic!("[gpu] {id} is no shader module"),
        }
    }

    pub(super) fn encoder(&mut self, id: u32) -> &mut wgpu::CommandEncoder {
        match self.get_mut(id) {
            Object::Encoder(e) => e,
            _ => panic!("[gpu] {id} is no encoder"),
        }
    }

    pub(super) fn render_pass(&mut self, id: u32) -> &mut wgpu::RenderPass<'static> {
        match self.get_mut(id) {
            Object::RenderPass(p) => p,
            _ => panic!("[gpu] {id} is no render pass"),
        }
    }

    pub(super) fn compute_pass(&mut self, id: u32) -> &mut wgpu::ComputePass<'static> {
        match self.get_mut(id) {
            Object::ComputePass(p) => p,
            _ => panic!("[gpu] {id} is no compute pass"),
        }
    }
}
