// pxl-kernel/domain/scene/character.rs

//! The characters a player can be, a person or a pet, as the character
//! kind's catalog lists them (`spec/spec/asset/character.json`); the two
//! the player chose are the scene's companion (`companion.rs`). A character
//! is its model, registered once at its height, playing its clips: what is
//! its own alone is its row, its clips' times and its joint palette, in a
//! block of its own, and where its walk stands it (`companion.rs` `Walk`).
//! Each tick advances its clips, cross-fades, and writes the
//! palette into its block. Each view draws it by its own eye: the world's as
//! decor's tiers do, by how much of a camera cell it spans, posed as it
//! plays down to the model's edge, its pose held below it down to the
//! impostor's, and nothing below that, as decor smaller than that is none
//! (`decor::gpu`), and nothing while the eye is the pivot; a character
//! surface's whole, before its own eye (`surface.rs`).

pub mod companion;
pub mod param;
pub mod surface;

use pxl_render::frame::shader::record::PIPELINE_SKINNED;
use pxl_render::gpu::api::{word_bytes, Image};
use pxl_render::gpu::draw::{scaled_draw, Draw, DrawId, DrawList, Position, Stand};
use pxl_spec::asset::catalog::Slot;
use pxl_spec::asset::character::groups::{person, pet};
pub use pxl_spec::asset::character::Group;
use pxl_spec::space::coord::v3;
use xpute_core::clock::now;
use xpute_core::mem;
use xpute_core::mem::heap::{alloc, free};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::domain::scene::animation;
use crate::domain::scene::character::companion::Walk;
use crate::domain::scene::decor::gpu::{decor_impostor_angle, decor_model_angle};
use crate::domain::scene::frame::Frame;
use crate::domain::scene::model::asset::model_of;
use crate::domain::scene::model::registry::{blend, Models};
use crate::domain::scene::model::{draw_list, models};
use crate::domain::scene::transform::Transform;
use crate::domain::view::lens::{cell_there, Stepped};
use crate::sys::io::loader::{load_image, load_of};
use crate::sys::sched::edge::wake_at;

pub const CHARACTERS: usize = person::models.len() + pet::models.len();

/// A character's model, registered once at its own height, which a
/// character surface's draw scales to a unit tall.
pub const MODELS_MAX: usize = CHARACTERS;

/// The row `number` of `group`: its model, its skin, its height in units and
/// its walk clip; none past the group's rows.
fn row(group: Group, number: u8) -> Option<(Slot, Slot, f64, u32)> {
    let n = usize::from(number);
    Some(match group {
        Group::person => (
            *person::models.get(n)?,
            person::skin,
            person::size,
            person::move_clip,
        ),
        Group::pet => (*pet::models.get(n)?, pet::skin, pet::size, pet::move_clip),
    })
}

#[derive(Clone, Copy)]
struct Playing {
    clip: u32,
    time: f32,
}

pub struct Character {
    group: Group,
    /// Its row in its group, as the settings store holds it.
    pub number: u8,
    /// Its model's number in `Models`.
    model: u32,
    cur: Option<Playing>,
    previous: Option<Playing>,
    fade: f32,
    fade_len: f32,
    palette_at: usize,
    palette_bytes: u32,
    palette_handle: u32,
    local: Vec<[f32; 10]>,
    global: Vec<[f32; 16]>,
    /// The fade's source pose, its room kept.
    from: Vec<[f32; 10]>,
    /// Its palette shows its clips at their clocks: posed again only where
    /// it moves where it is seen, or where it does not show them.
    posed: bool,
    /// Whether it spans the impostor's edge at the last tick: drawn at all.
    drawn: bool,
    /// How long its clip has run while it moves it where it is seen, in
    /// seconds: by the wall clock between its ticks, read at `last`, a stall
    /// taken as one step.
    run: f64,
    last: f64,
    /// Its clip's clock as drawn: `run` stepped a pixel's motion at a time
    /// (`lens::Stepped`), and the reading its clip last took (`clip_time`).
    clock: Stepped,
    clip_time: f64,
    /// Its draw in the world's list.
    id: DrawId,
    /// Where it stands in the world, as its walk last stood it; none while
    /// the eye is the pivot.
    transform: Option<Transform>,
    walk: Walk,
}

impl Character {
    /// The row `number` of `group` once its model, registered at its height,
    /// and its skin have landed, both asked for each time; none meanwhile,
    /// past the group's rows, or where the heap refuses.
    pub fn of(group: Group, number: u8) -> Option<Character> {
        let (slot, skin, height_u, _) = row(group, number)?;
        let (model, skin) = (
            model_of(slot, Some(height_u)),
            load_of(skin).and_then(load_image),
        );
        let (model, _) = (model?, skin?);
        Character::new(draw_list(), models(), model, group, number)
    }

    /// The row `number` of `group` drawn as `model`, its palette's block
    /// taken from the heap; none where the model is not registered or the
    /// heap refuses.
    fn new(
        draw: &mut DrawList,
        models: &Models,
        model: u32,
        group: Group,
        number: u8,
    ) -> Option<Character> {
        let joints = models.get(model)?.joints();
        let palette_bytes = word_bytes(joints.max(1) * 16);
        let (palette_at, palette_bytes) = alloc(palette_bytes)?;
        // SAFETY: the block just acquired; identity joints until the first tick.
        unsafe {
            let p = core::slice::from_raw_parts_mut(
                mem::at::<f32>(palette_at),
                palette_bytes as usize / 4,
            );
            p.fill(0.0);
            for j in 0..joints.max(1) {
                for d in 0..4 {
                    p[j * 16 + d * 5] = 1.0;
                }
            }
        }
        let palette_handle = draw.upload(palette_at as u32, palette_bytes, 0, 0);
        Some(Character {
            group,
            number,
            model,
            cur: None,
            previous: None,
            fade: 0.0,
            fade_len: 0.0,
            palette_at,
            palette_bytes,
            palette_handle,
            local: vec![[0.0; 10]; joints],
            global: vec![[0.0; 16]; joints],
            from: Vec::new(),
            posed: false,
            drawn: false,
            run: 0.0,
            last: 0.0,
            clock: Stepped::default(),
            clip_time: 0.0,
            id: DrawId::default(),
            transform: None,
            walk: Walk::new([0.0; 2]),
        })
    }

    /// Its skin as the renderer takes it, asked of the loader; none while it
    /// loads.
    pub fn skin(&self) -> Option<Image> {
        load_of(self.row().1).and_then(load_image)
    }

    /// Its height in units: a cell for the person, a quarter for the pet.
    pub fn height_u(&self) -> f64 {
        self.row().2
    }

    /// Its walk clip, by its animation's index.
    pub fn move_clip(&self) -> u32 {
        self.row().3
    }

    fn row(&self) -> (Slot, Slot, f64, u32) {
        row(self.group, self.number).or_bug(Errno::ENOTRECOVERABLE)
    }

    /// Fades from the clip playing into `clip`, over `fade_s` where one
    /// plays. A clip its model lacks, or one already playing, is left alone.
    pub fn play(&mut self, models: &Models, clip: u32, fade_s: f32) {
        let lacks = models.get(self.model).is_none_or(|m| clip >= m.clip_cnt());
        if lacks || self.cur.is_some_and(|c| c.clip == clip) {
            return;
        }
        self.previous = self.cur;
        self.cur = Some(Playing { clip, time: 0.0 });
        self.posed = false;
        self.fade = 0.0;
        self.fade_len = if self.previous.is_some() {
            fade_s.max(0.0)
        } else {
            0.0
        };
    }

    /// Its block given back.
    pub fn free(self, draw: &mut DrawList) {
        draw.free(self.palette_handle);
        // SAFETY: the block this instance took and has not given back.
        unsafe { free((self.palette_at, self.palette_bytes)) };
    }

    /// Its clock advanced; true where a view can see it move. The world sees
    /// it where the walk stands it, in its view and a cell of the camera grid
    /// across or more, as the rays observe nothing smaller, or fading from
    /// one clip into the next. So a frame is asked only for what can be seen
    /// to change. A clip moving it less than a cell holds its time and its
    /// pose until the eye comes close enough to see it move. One moving it a
    /// cell or more steps where any point of it may have gone a pixel of the
    /// display from where it was drawn, or at the next key where its motion
    /// turns back, and asks no frame: the turn is woken for the next step and
    /// draws it, as the location indicator's bob is (`Model::next_step`).
    /// A character surface showing it (`on_surface`) sees it whole before its own eye, its clips
    /// running a frame a step and a frame asked for its turn (`surface_draw`).
    pub fn tick(
        &mut self,
        draw: &mut DrawList,
        models: &Models,
        frame: &Frame,
        on_surface: bool,
    ) -> bool {
        let mut dt = frame.delta_s as f32;
        let Some(m) = models.get(self.model) else {
            return false;
        };
        let length = |p: &Playing| m.duration(p.clip).filter(|&d| d > 0.0);
        // A clip of some length plays, or one fades into the next.
        let playing = self.cur.filter(|p| length(p).is_some());
        let moving = playing.is_some() || self.previous.is_some();
        // The world's view: what spans the model's edge at its distance is
        // observed, as a decor's model is; what spans the impostor's is
        // drawn, posed or not, as a decor's impostor is.
        let (mut seen, mut world_moves, mut dist, mut scale) = (false, false, 0.0, 1.0);
        self.drawn = false;
        if let Some(t) = self.transform {
            scale = t.scale * m.scale();
            let about = v3::sub(&t.at, &frame.anchor);
            let r = f64::from(m.pose_radius(self.cur.map(|p| p.clip)) * scale);
            dist = v3::len(&v3::sub(&about, &frame.camera));
            let cell_there = cell_there(dist);
            seen = frame.frustum.sees(about, r) && 2.0 * r >= decor_model_angle() * dist;
            self.drawn = 2.0 * r >= decor_impostor_angle() * dist;
            world_moves = seen
                && (self.previous.is_some()
                    || playing.is_some_and(|p| f64::from(m.motion(p.clip) * scale) >= cell_there));
        }
        let mut moves = world_moves || on_surface && moving;
        let mut asks = moves || on_surface;
        // A clip alone, seen to move, steps where its pose has gone a pixel
        // of the display from the one drawn, or turns back; a fade runs a
        // frame a step.
        if let Some((p, d)) = playing
            .filter(|_| world_moves && !on_surface && self.previous.is_none())
            .and_then(|p| Some((p, f64::from(length(&p)?))))
        {
            let far = (dist / frame.px_per_unit / f64::from(scale)) as f32;
            let (start, shown) = (f64::from(p.time), self.clip_time);
            // The clip's time at a reading `at` of `run`, and the reading it
            // next steps at from there.
            let step = |at: f64| {
                let t = (start + at - shown).rem_euclid(d) as f32;
                at + f64::from(m.next_step(p.clip, t, far) - t)
            };
            let wall = now();
            let since = if self.last > 0.0 {
                (wall - self.last) / 1000.0
            } else {
                frame.delta_s
            };
            self.last = wall;
            self.run += since.clamp(0.0, (step(self.run) - self.run).max(frame.delta_s));
            let (time, next, stepped) = self.clock.at(self.run, step);
            if next.is_finite() {
                wake_at(wall + (next - self.run) * 1000.0);
            }
            dt = (time - self.clip_time) as f32;
            self.clip_time = time;
            (moves, asks) = (stepped, false);
            if stepped {
                animation::note_animation();
            }
        } else {
            self.last = 0.0;
        }
        let wrap = |p: &mut Playing| {
            p.time += dt;
            if let Some(d) = length(p) {
                p.time %= d;
            }
        };
        // Seen, its clip holds where its motion is too small to see.
        let seen = seen || on_surface;
        let runs = moves || !seen;
        if runs {
            if let Some(p) = self.cur.as_mut() {
                wrap(p);
            }
            if let Some(p) = self.previous.as_mut() {
                wrap(p);
            }
            if self.previous.is_some() {
                self.fade += dt;
                if self.fade >= self.fade_len {
                    self.previous = None;
                }
            }
        }
        let joints = m.joints();
        if joints == 0 {
            return asks;
        }
        // Unseen, its clocks run on and its palette waits for it to be
        // seen; held, its pose stands as it was.
        if !moves && (self.posed || !seen) {
            self.posed &= !(moving && runs);
            return asks;
        }
        self.posed = true;
        m.pose(self.cur.map(|p| (p.clip, p.time)), &mut self.local);
        if let Some(prev) = self.previous {
            self.from.resize(joints, [0.0; 10]);
            m.pose(Some((prev.clip, prev.time)), &mut self.from);
            let w = if self.fade_len > 0.0 {
                (self.fade / self.fade_len).clamp(0.0, 1.0)
            } else {
                1.0
            };
            for (to, from) in self.local.iter_mut().zip(self.from.iter()) {
                *to = blend(from, to, w);
            }
        }
        // SAFETY: the character's own palette block, one mat4 a joint.
        let palette = unsafe {
            core::slice::from_raw_parts_mut(mem::at::<f32>(self.palette_at), joints * 16)
        };
        m.palette(&self.local, &mut self.global, palette);
        draw.rewrite(
            self.palette_handle,
            self.palette_at as u32,
            self.palette_bytes,
        );
        asks
    }

    /// Into the world's list, where the walk stands it and it spans the
    /// impostor's edge (`tick`), within the ball about it (`Stand::ball`),
    /// its draw and its stand free of the eye: every character the world
    /// draws is a receiver of the sun's map, which reads the ones the eye
    /// sees.
    pub fn draw(&self, draw: &mut DrawList, models: &Models) {
        let Some(m) = models.get(self.model).filter(|m| m.handle() != 0) else {
            return;
        };
        let Some(t) = self.transform.filter(|_| self.drawn) else {
            return;
        };
        draw.draw_scaled(
            &self.id,
            PIPELINE_SKINNED,
            m.handle(),
            self.palette_handle,
            self.model,
            Position::of(t.at),
            t.rot,
            1.0,
            t.scale * m.scale(),
            Some(Stand::ball(t.at)),
        );
    }

    /// Its draw as a character surface's eye draws it, held by no list: where the walk
    /// stands it, or anywhere while it stands nowhere, as the eye stands at
    /// its point; its model a unit tall, as the eye frames it (pxl-render
    /// gpu/pass/character.rs), turned by the clock so every side of it shows.
    /// None while its model is not uploaded.
    pub fn surface_draw(&self, models: &Models) -> Option<Draw> {
        let m = models.get(self.model).filter(|m| m.handle() != 0)?;
        let at = self.transform.map_or([0.0; 3], |t| t.at);
        let rot = (now() / 1000.0 * param::SURFACE_SPIN_RAD_S % core::f64::consts::TAU) as f32;
        Some(scaled_draw(
            PIPELINE_SKINNED,
            m.handle(),
            self.palette_handle,
            self.model,
            Position::of(at),
            rot,
            1.0,
            m.scale() / self.height_u() as f32,
        ))
    }
}

#[cfg(test)]
#[path = "character.test.rs"]
mod test;
