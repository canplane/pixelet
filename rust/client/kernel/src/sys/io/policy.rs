// pxl-kernel/sys/io/policy.rs

//! The kernel's serve policy: the one place every producer's vocabulary is
//! ranked against the others. It changes how soon the view is right, never
//! what it is.
//!
//! 1. By layer, the order a view builds up: terrain, then the pictures before
//!    the models (a picture, the Earth's sphere, the moon's face, the ground's
//!    pack or a skin, is drawn whole wherever it shows; a model is one thing
//!    decor draws), the Earth's tiles with the pictures as the far view's,
//!    then the catalog's models, then places. The objects' shards ride the ground's
//!    order: each of the ground's shells, seen then unseen, is followed by the
//!    objects' of the same shell, so what is placed in view comes before
//!    ground out of it.
//! 2. Within a layer, seen before unseen and nearer first;
//!    of tiles as near, the finer first, so the leaf under the eye is not
//!    asked after the coarse tiles over it, which cost a fold where baked on demand.
//!
//! Everything else ties, after the view. Rank orders what starts; nothing
//! running is stopped for it.

use crate::sys::io::demand::{Order, Stream};
use crate::sys::io::failure::Failure;
use crate::sys::io::loader::of_id;
use crate::sys::io::param;
use crate::sys::io::request::{Fault, Rank, Serve};
use crate::sys::load::LoadKind;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::math::rng::{u32_to_unit, SplitMix64};
use xpute_kit::status::errno::Errno;

/// A layer's classes: seen or unseen.
const SHELLS: u8 = 2;
/// Terrain and the objects interleave by shell: a shell's terrain, then its
/// objects (`GROUND_SHELL` apart).
const TERRAIN: u8 = 0;
const OBJECT: u8 = TERRAIN + 1;
const GROUND_SHELL: u8 = 2;
/// The pictures, and the Earth's tiles seen then unseen: what decides it is a
/// picture's or a model's file, not which file.
const PICTURE: u8 = TERRAIN + SHELLS * GROUND_SHELL;
const ASSET: u8 = PICTURE + SHELLS;
const PLACE: u8 = ASSET + 1;
const ASIDE: u8 = PLACE + SHELLS;

pub struct Policy {
    pub intake: u32,
}

impl Policy {
    pub const fn standard() -> Policy {
        Policy {
            intake: param::INTAKE,
        }
    }
}

fn tile(layer: Stream, id: u64, order: Order) -> Rank {
    if order == Order::LAST {
        return Rank {
            class: ASIDE,
            dist: 0.0,
            detail: 0,
        };
    }
    let unseen = !order.seen as u8;
    // A zoom fits the rank's byte. The objects' ids are the top's, no tile's,
    // and shards' all at `SHARD_ZOOM`, which no zoom orders.
    let zoom = || Quadrant::of_id(id).zoom as u8;
    let (class, detail) = match layer {
        Stream::terrain => (TERRAIN + unseen * GROUND_SHELL, zoom()),
        Stream::object => (OBJECT + unseen * GROUND_SHELL, 0),
        Stream::earth => (PICTURE + unseen, zoom()),
        _ => (PLACE + unseen, zoom()),
    };
    Rank {
        class,
        dist: order.near,
        detail,
    }
}

impl Serve for Policy {
    fn rank(&self, stream: Stream, id: u64, order: Order, _enqueue_ms: f64) -> Rank {
        match stream {
            Stream::terrain | Stream::object | Stream::place | Stream::earth => {
                tile(stream, id, order)
            }
            Stream::asset => Rank {
                class: match of_id(id) {
                    Some((LoadKind::Image, _)) => PICTURE,
                    _ => ASSET,
                },
                dist: 0.0,
                detail: 0,
            },
            Stream::division => Rank {
                class: ASIDE,
                dist: 0.0,
                detail: 0,
            },
        }
    }

    fn intake(&self) -> u32 {
        self.intake
    }

    /// As the client's read classes them (`ts/client/src/sys/fs/read.ts`). An object's
    /// failure comes back the same however often it is asked: the bytes are
    /// fixed by the key within a version.
    fn fault(&self, failure: Failure) -> Fault {
        match failure.errnum {
            // The browser says it is offline.
            Errno::ENETUNREACH => Fault::Network,
            // Any other status but 204 and 404, or no response while online. The
            // Worker answers only 200, 204 and 404, so this is the platform under
            // it: R2's internal error, a Cloudflare incident, the free plan's
            // daily cap failing closed, an edge refusing the connection.
            Errno::EACCES | Errno::EIO => Fault::Server,
            // ENXIO: 404, a key that names nothing. EBADMSG: a body empty or not
            // decoding, one still packed in an encoding this browser does not undo
            // among them.
            // EFBIG: larger than the heap takes, or than its kind's packet can be.
            _ => Fault::Object,
        }
    }

    /// Exponential because the server is shared, so every client's retries add up
    /// on it; AWS's "full jitter", uniform up to the capped exponential, so
    /// clients one outage failed together do not return together.
    fn backoff(&self, failures: u32, seed: u64) -> Option<f64> {
        if failures > param::RETRY_ATTEMPTS {
            return None;
        }
        let ceiling =
            (param::RETRY_BASE_MS * libm::exp2(f64::from(failures - 1))).min(param::RETRY_CAP_MS);
        Some(ceiling * u32_to_unit(SplitMix64::new(seed).next_u32()))
    }
}

#[cfg(test)]
#[path = "policy.test.rs"]
mod test;
