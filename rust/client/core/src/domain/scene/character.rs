// pxl-core/domain/scene/character.rs

//! The characters a player can be, a person or a pet, as the character
//! catalog lists them (`spec/spec/asset/character.json`). The two the player
//! chose make up the scene's companion (`companion.rs`).
//!
//! A character plays the clips of a model registered once at the character's
//! height. What belongs to one character alone is its catalog row, its clips'
//! times, its joint palette in a heap block of its own, and the place the
//! companion's walk gives it (`companion.rs`).
//!
//! Each view draws a character by its own eye. The world chooses as it does
//! for decor's lods (`view::lens`), by how many camera grid cells the
//! character spans. Above the model's edge it is posed as it plays. Between
//! the model's edge and the impostor's its pose is held. Below the impostor's
//! edge it is not drawn, as decor that small is not, and it is not drawn while
//! the eye is the pivot. A character surface draws it whole, before the
//! surface's own eye (`surface.rs`).

pub mod companion;
pub mod param;
pub mod surface;

use pxl_render::frame::shader::record::PIPELINE_SKINNED;
use pxl_render::gpu::api::{word_bytes, Image};
use pxl_render::gpu::draw::{scaled_draw, Draw, DrawId, DrawList, Position, ReceiverBox};
use pxl_spec::asset::catalog::Slot;
use pxl_spec::asset::character::groups::{person, pet};
pub use pxl_spec::asset::character::Group;
use pxl_spec::space::coord::v3;
use xpute_guest::clock::now;
use xpute_link::mem;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::domain::scene::animation;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::model::asset::model_of;
use crate::domain::scene::model::registry::{blend, ModelTable};
use crate::domain::scene::model::{draw_list, models};
use crate::domain::scene::transform::Transform;
use crate::domain::view::lens::{cell_there, decor_impostor_angle, decor_model_angle, StepClock};
use crate::sys::heap;
use crate::sys::io::loader::{load_image, load_of};
use crate::sys::sched::invalidate_at;

pub const CHARACTERS: usize = person::models.len() + pet::models.len();

/// One model a character, registered once at the character's own height. A
/// character surface's draw scales the model to one unit tall.
pub const MODELS_MAX: usize = CHARACTERS;

/// The catalog row `number` of `group`: its model, its skin, its height in
/// units and its walk clip. None past the group's last row.
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
struct ClipTime {
    clip: u32,
    time: f32,
}

pub struct Character {
    group: Group,
    /// The character's row in its group, as the settings store holds it.
    pub number: u8,
    /// The model's index in `ModelTable`.
    model: u32,
    cur: Option<ClipTime>,
    previous: Option<ClipTime>,
    fade: f32,
    fade_len: f32,
    /// The joints' palette, one mat4 a joint, which the renderer uploads.
    palette: Box<[f32]>,
    palette_handle: u32,
    local: Vec<[f32; 10]>,
    global: Vec<[f32; 16]>,
    /// The pose the fade starts from. Its allocation is kept between fades.
    from: Vec<[f32; 10]>,
    /// Whether the palette shows the clips at their current times. The
    /// character is posed again only when it moves where it is seen, or when
    /// the palette does not show its clips.
    posed: bool,
    /// Whether the character spanned the impostor's edge at the last tick,
    /// which decides whether it is drawn at all.
    visible: bool,
    /// How long, in seconds, the clip has run while it moves the character
    /// where it is seen. It advances by the wall clock between ticks, read at
    /// `last`, and a stall counts as one step.
    run: f64,
    last: f64,
    /// The clip's clock as drawn: `run` stepped one pixel's motion at a time
    /// (`lens::StepClock`). `clip_time` is the reading the clip last took.
    clock: StepClock,
    clip_time: f64,
    /// The character's draw in the world's list.
    id: DrawId,
    /// Where the character's walk last placed it in the world. None while the
    /// eye is the pivot.
    transform: Option<Transform>,
}

impl Character {
    /// The character of row `number` of `group`, once its model, registered
    /// at its height, and its skin have loaded. Each call asks for both again.
    /// None while they load, past the group's last row, or when the heap
    /// refuses.
    pub fn of(group: Group, number: u8) -> Option<Character> {
        let (slot, skin, height_u, _) = row(group, number)?;
        let (model, skin) = (
            model_of(slot, Some(height_u)),
            load_of(skin).and_then(load_image),
        );
        let (model, _) = (model?, skin?);
        Character::new(draw_list(), models(), model, group, number)
    }

    /// The character of row `number` of `group`, drawn as `model`, with its
    /// palette taken from the heap. None when the model is not
    /// registered or the heap refuses.
    fn new(
        draw: &mut DrawList,
        models: &ModelTable,
        model: u32,
        group: Group,
        number: u8,
    ) -> Option<Character> {
        let joints = models.get(model)?.joints();
        // Identity joints until the first tick.
        let mut palette = heap::zeroed::<f32>(joints.max(1) * 16)?;
        for j in 0..joints.max(1) {
            for d in 0..4 {
                palette[j * 16 + d * 5] = 1.0;
            }
        }
        let palette_handle = draw.upload(
            mem::off(palette.as_ptr().cast()),
            word_bytes(palette.len()),
            0,
            0,
        );
        Some(Character {
            group,
            number,
            model,
            cur: None,
            previous: None,
            fade: 0.0,
            fade_len: 0.0,
            palette,
            palette_handle,
            local: vec![[0.0; 10]; joints],
            global: vec![[0.0; 16]; joints],
            from: Vec::new(),
            posed: false,
            visible: false,
            run: 0.0,
            last: 0.0,
            clock: StepClock::default(),
            clip_time: 0.0,
            id: DrawId::default(),
            transform: None,
        })
    }

    /// The character's skin as the renderer takes it. None while it loads.
    pub fn skin(&self) -> Option<Image> {
        load_of(self.row().1).and_then(load_image)
    }

    /// The character's height in units: a cell for the person, a quarter of
    /// one for the pet.
    pub fn height_u(&self) -> f64 {
        self.row().2
    }

    /// The index of the character's walk clip among its model's animations.
    pub fn move_clip(&self) -> u32 {
        self.row().3
    }

    fn row(&self) -> (Slot, Slot, f64, u32) {
        row(self.group, self.number).or_bug(Errno::ENOTRECOVERABLE)
    }

    /// Fades from the clip playing into `clip` over `fade_s` seconds. With no
    /// clip playing, `clip` starts at once. A clip the model lacks, or the
    /// clip already playing, changes nothing.
    pub fn play(&mut self, models: &ModelTable, clip: u32, fade_s: f32) {
        let lacks = models.get(self.model).is_none_or(|m| clip >= m.clip_cnt());
        if lacks || self.cur.is_some_and(|c| c.clip == clip) {
            return;
        }
        self.previous = self.cur;
        self.cur = Some(ClipTime { clip, time: 0.0 });
        self.posed = false;
        self.fade = 0.0;
        self.fade_len = if self.previous.is_some() {
            fade_s.max(0.0)
        } else {
            0.0
        };
    }

    pub fn free(self, draw: &mut DrawList) {
        draw.free(self.palette_handle);
    }

    /// Advances the character's clips and returns whether a frame must be
    /// asked for. A frame is asked for only a change a view can see.
    ///
    /// The world sees the character move when it stands in view, spans the
    /// model's edge, and either fades from one clip into the next or plays a
    /// clip that carries some point of it a camera grid cell or more; the
    /// rays observe nothing smaller. A clip that moves it less than a cell
    /// holds its time and its pose until the eye comes close enough to see
    /// the motion. A clip that moves it a cell or more steps when any point
    /// of it may have gone a display pixel from where it was drawn, or at the
    /// next key where its motion turns back (`Model::next_step`). Such a step
    /// asks for no frame: the frame is woken at the next step and draws it, as
    /// it is for the location indicator's bob.
    ///
    /// A character surface that shows the character (`on_surface`) sees it
    /// whole, before the surface's own eye (`surface_draw`). There its clips
    /// run a frame a step, and every frame asks for a frame.
    pub fn tick(
        &mut self,
        draw: &mut DrawList,
        models: &ModelTable,
        frame: &Frame,
        on_surface: bool,
    ) -> bool {
        let mut dt = frame.delta_s as f32;
        let Some(m) = models.get(self.model) else {
            return false;
        };
        let length = |p: &ClipTime| m.duration(p.clip).filter(|&d| d > 0.0);
        let playing = self.cur.filter(|p| length(p).is_some());
        let moving = playing.is_some() || self.previous.is_some();
        // In the world, the character is observed where it spans the model's
        // edge at its distance, as a decor's model is. It is drawn, posed or
        // not, where it spans the impostor's edge, as a decor's impostor is.
        let (mut seen, mut world_moves, mut dist, mut scale) = (false, false, 0.0, 1.0);
        self.visible = false;
        if let Some(t) = self.transform {
            scale = t.scale * m.scale();
            let about = v3::sub(&t.at, &frame.anchor);
            let r = f64::from(m.pose_radius(self.cur.map(|p| p.clip)) * scale);
            dist = v3::len(&v3::sub(&about, &frame.camera));
            let cell_there = cell_there(dist);
            seen = frame.frustum.sees(about, r) && 2.0 * r >= decor_model_angle() * dist;
            self.visible = 2.0 * r >= decor_impostor_angle() * dist;
            world_moves = seen
                && (self.previous.is_some()
                    || playing.is_some_and(|p| f64::from(m.motion(p.clip) * scale) >= cell_there));
        }
        let mut moves = world_moves || on_surface && moving;
        let mut asks = moves || on_surface;
        // A clip without a fade, seen to move, steps when its pose may have
        // gone a display pixel from the pose drawn, or when its motion turns
        // back. A fade runs one frame a step.
        if let Some((p, d)) = playing
            .filter(|_| world_moves && !on_surface && self.previous.is_none())
            .and_then(|p| Some((p, f64::from(length(&p)?))))
        {
            let far = (dist / frame.px_per_unit / f64::from(scale)) as f32;
            let (start, shown) = (f64::from(p.time), self.clip_time);
            // For a reading `at` of `run`, the reading at which the clip next
            // steps.
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
                invalidate_at(wall + (next - self.run) * 1000.0);
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
        let wrap = |p: &mut ClipTime| {
            p.time += dt;
            if let Some(d) = length(p) {
                p.time %= d;
            }
        };
        // A seen character's clip holds while its motion is too small to see.
        // An unseen character's clip runs on.
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
        // An unseen character's clocks run on while its palette waits until
        // it is seen. A held character keeps the pose it has.
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
        m.palette(
            &self.local,
            &mut self.global,
            &mut self.palette[..joints * 16],
        );
        draw.rewrite(
            self.palette_handle,
            mem::off(self.palette.as_ptr().cast()),
            word_bytes(self.palette.len()),
        );
        asks
    }

    /// Adds the character to the world's list where its walk placed it, when
    /// it spans the impostor's edge (`tick`). Neither its draw nor its
    /// receiver box (`ReceiverBox::ball`) depends on the eye. Every character
    /// the world draws receives the sun's shadow map, and the map covers the
    /// receivers the eye sees.
    pub fn draw(&self, draw: &mut DrawList, models: &ModelTable) {
        let Some(m) = models.get(self.model).filter(|m| m.handle() != 0) else {
            return;
        };
        let Some(t) = self.transform.filter(|_| self.visible) else {
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
            t.scale * m.scale(),
            Some(ReceiverBox::ball(t.at)),
        );
    }

    /// The character's draw for a character surface, which no list holds.
    /// The character stands where its walk placed it, or at the origin while
    /// it has no place; the surface's eye stands at that point either way.
    /// The model is scaled to one unit tall, as the surface's eye frames it
    /// (pxl-render pass/character.rs), and turned by the clock so that
    /// every side shows. None while the model is not uploaded.
    pub fn surface_draw(&self, models: &ModelTable) -> Option<Draw> {
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
            m.scale() / self.height_u() as f32,
        ))
    }
}

#[cfg(test)]
#[path = "character.test.rs"]
mod test;
