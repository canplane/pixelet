// pxl-kernel/scene/actor.rs

//! The actors that play registered models. Each tick advances every actor's
//! clips, cross-fades as three's mixer did, and writes the joint palette into
//! the actor's block.

use pxl_render::frame::draw::{DrawList, PIPELINE_SKINNED};
use pxl_render::gpu::api::word_bytes;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::abi::handle::Slots;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};

use crate::model::registry::{blend, Models};

/// The companion's and the display cases'.
pub const ACTORS_MAX: usize = crate::scene::character::companion::ACTORS_MAX + crate::scene::character::preview::CASES_MAX;

#[derive(Clone, Copy)]
struct Playing {
    clip: u32,
    time: f32,
}

pub struct Actor {
    model: u32,
    /// In units, and its turn about Z.
    pub at: V3,
    pub rotation_z: f32,
    pub visible: bool,
    current: Option<Playing>,
    previous: Option<Playing>,
    fade: f32,
    fade_len: f32,
    palette_at: usize,
    palette_bytes: u32,
    palette_handle: u32,
    local: Vec<[f32; 10]>,
    global: Vec<[f32; 16]>,
}

#[derive(Default)]
pub struct Actors {
    actors: Slots<Actor>,
    from: Vec<[f32; 10]>,
}

impl Actors {
    pub fn new_actor(&mut self, draw: &mut DrawList, models: &Models, model: u32) -> Option<u32> {
        xpute_core::ensure!(self.actors.iter().count() < ACTORS_MAX, ENOTRECOVERABLE);
        let joints = models.get(model)?.joints();
        let palette_bytes = word_bytes(joints.max(1) * 16);
        let (palette_at, palette_bytes) = alloc(palette_bytes)?;
        // SAFETY: the block just acquired; identity joints until the first tick.
        unsafe {
            let p = core::slice::from_raw_parts_mut(ptr_at::<f32>(palette_at), palette_bytes as usize / 4);
            p.fill(0.0);
            for j in 0..joints.max(1) {
                for d in 0..4 {
                    p[j * 16 + d * 5] = 1.0;
                }
            }
        }
        let palette_handle = draw.upload(palette_at as u32, palette_bytes, 0, 0);
        let actor = Actor {
            model,
            at: [0.0; 3],
            rotation_z: 0.0,
            visible: false,
            current: None,
            previous: None,
            fade: 0.0,
            fade_len: 0.0,
            palette_at,
            palette_bytes,
            palette_handle,
            local: vec![[0.0; 10]; joints],
            global: vec![[0.0; 16]; joints],
        };
        Some(self.actors.insert(actor))
    }

    pub fn actor(&mut self, id: u32) -> Option<&mut Actor> {
        self.actors.get_mut(id)
    }

    pub fn actor_ref(&self, id: u32) -> Option<&Actor> {
        self.actors.get(id)
    }

    /// A clip already playing is left alone.
    pub fn play(&mut self, id: u32, clip: u32, fade_s: f32) {
        let Some(a) = self.actor(id) else { return };
        if a.current.is_some_and(|c| c.clip == clip) {
            return;
        }
        a.previous = a.current;
        a.current = Some(Playing { clip, time: 0.0 });
        a.fade = 0.0;
        a.fade_len = if a.previous.is_some() { fade_s.max(0.0) } else { 0.0 };
    }

    pub fn remove_actor(&mut self, draw: &mut DrawList, id: u32) {
        if let Some(a) = self.actors.remove(id) {
            draw.free(a.palette_handle);
            // SAFETY: the block this asset took and has not given back.
            unsafe { free((a.palette_at, a.palette_bytes)) };
        }
    }

    /// Whether any actor plays a clip of some length: the only thing that asks
    /// for the next frame.
    pub fn tick(&mut self, draw: &mut DrawList, models: &Models, dt: f32) -> bool {
        let mut animating = false;
        let Actors { actors, from } = self;
        for a in actors.values_mut() {
            let Some(m) = models.get(a.model) else { continue };
            let length = |p: &Playing| m.duration(p.clip).filter(|&d| d > 0.0);
            animating |= a.current.as_ref().and_then(length).is_some();
            let wrap = |p: &mut Playing| {
                p.time += dt;
                if let Some(d) = length(p) {
                    p.time %= d;
                }
            };
            if let Some(p) = a.current.as_mut() {
                wrap(p);
            }
            if let Some(p) = a.previous.as_mut() {
                wrap(p);
            }
            if a.previous.is_some() {
                a.fade += dt;
                if a.fade >= a.fade_len {
                    a.previous = None;
                }
            }
            let joints = m.joints();
            if joints == 0 {
                continue;
            }
            m.pose(a.current.map(|p| (p.clip, p.time)), &mut a.local);
            if let Some(prev) = a.previous {
                from.resize(joints, [0.0; 10]);
                m.pose(Some((prev.clip, prev.time)), from);
                let w = if a.fade_len > 0.0 { (a.fade / a.fade_len).clamp(0.0, 1.0) } else { 1.0 };
                for (to, from) in a.local.iter_mut().zip(from.iter()) {
                    *to = blend(from, to, w);
                }
            }
            // SAFETY: the actor's own palette block, one mat4 a joint.
            let palette = unsafe { core::slice::from_raw_parts_mut(ptr_at::<f32>(a.palette_at), joints * 16) };
            m.palette(&a.local, &mut a.global, palette);
            draw.rewrite(a.palette_handle, a.palette_at as u32, a.palette_bytes);
        }
        animating
    }

    pub fn draw(&self, draw: &mut DrawList, models: &Models, eye: V3) {
        for (_, a) in self.actors.iter() {
            if !a.visible {
                continue;
            }
            let Some(m) = models.get(a.model) else { continue };
            if m.handle() == 0 {
                continue;
            }
            let t = DrawList::translation(a.at, eye);
            draw.draw_scaled(PIPELINE_SKINNED, m.handle(), a.palette_handle, a.model, t, a.rotation_z, 1.0, m.scale());
        }
    }
}
