// pxl-core/sys/io/policy.rs

//! The core's serve policy: the one place where the wants of every producer
//! are ranked against each other. The rank changes how soon the view is
//! right, never what the view shows. A layer's wants fall into two shells:
//! those seen and those unseen.
//!
//! 1. Layers rank in the order a view builds up. Terrain comes first. The
//!    pictures come before the models, because a picture (the imagery's global
//!    picture, the moon's face, the ground's pack or a skin) is drawn whole
//!    wherever it shows, while a model is one of the things decor draws. The
//!    imagery's tiles rank with the pictures, since they are the far view's
//!    picture. The catalog's models come next, then places. The object shards
//!    follow the ground's order: each shell of the ground, seen and then
//!    unseen, is followed by the object shards of the same shell, so what is
//!    placed in view comes before ground out of view.
//! 2. Within a layer, seen comes before unseen, and nearer before farther.
//!    Among tiles equally near, the finer comes first, so the leaf under the
//!    eye is not asked for after the coarse tiles over it, each of which
//!    costs a fold where tiles are baked on demand.
//!
//! A want with no place in the view ranks after every other, and such wants
//! tie. The rank orders what starts; nothing already running is stopped for
//! it. A stream's row decides which class its wants take (sys/io/demand.rs
//! `Stream::row`), and this module decides how the classes rank.

use crate::sys::io::demand::{Class, Order, Stream};
use crate::sys::io::failure::Failure;
use crate::sys::io::param;
use crate::sys::load::{LoadKind, of_id};
use core::cmp::Ordering;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::math::rng::{SplitMix64, u32_to_unit};
use xpute_kit::status::errno::Errno;

/// The shells of a layer, seen and unseen, each a class of its own.
const SHELL_CNT: u8 = 2;
/// Terrain and the object layer interleave by shell: the terrain of one shell,
/// then the object shards of the same shell. The classes of one shell are
/// `GROUND_SHELL` from those of the next.
const TERRAIN: u8 = 0;
const OBJECT: u8 = TERRAIN + 1;
const GROUND_SHELL: u8 = 2;
/// The pictures, and the imagery's tiles seen then unseen. A file ranks here or
/// at `ASSET` by whether it is a picture or a model, never by which file it
/// is.
const PICTURE: u8 = TERRAIN + SHELL_CNT * GROUND_SHELL;
const ASSET: u8 = PICTURE + SHELL_CNT;
const PLACE: u8 = ASSET + 1;
const ASIDE: u8 = PLACE + SHELL_CNT;

/// The rank of a want. Wants start in order of a lower `class`, then a lower
/// `dist`, then a higher `detail`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rank {
    pub class: u8,
    pub dist: f64,
    pub detail: u8,
}

impl Rank {
    pub(crate) fn cmp(&self, other: &Rank) -> Ordering {
        self.class
            .cmp(&other.class)
            .then(self.dist.total_cmp(&other.dist))
            .then(other.detail.cmp(&self.detail))
    }
}

/// Who is at fault for a failure. The fault decides what waits on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    /// The object itself is at fault, and asking again brings the same
    /// failure.
    Object,
    /// There is no way to the server. The reads wait for the network and are
    /// never given up.
    Network,
    /// The server is at fault. Each such failure counts toward giving up.
    Server,
}

pub(crate) fn rank(stream: Stream, id: u64, order: Order) -> Rank {
    let unseen = !order.seen as u8;
    // A zoom fits in the rank's byte. The zoom is read only for streams whose
    // ids are tiles. An object's id is the top's, which is no tile, or a
    // shard's, and every shard is at `SHARD_ZOOM`, so a zoom orders none of
    // them.
    let zoom = || Quadrant::of_id(id).zoom as u8;
    let (class, dist, detail) = match stream.row().class {
        Class::File => match of_id(id) {
            Some((LoadKind::Image, _)) => (PICTURE, 0.0, 0),
            _ => (ASSET, 0.0, 0),
        },
        Class::Aside => (ASIDE, 0.0, 0),
        // `Order::LAST` marks a want with no place in the view.
        _ if order == Order::LAST => (ASIDE, 0.0, 0),
        Class::Ground => (TERRAIN + unseen * GROUND_SHELL, order.near, zoom()),
        Class::OnGround => (OBJECT + unseen * GROUND_SHELL, order.near, 0),
        Class::Picture => (PICTURE + unseen, order.near, zoom()),
        Class::Place => (PLACE + unseen, order.near, zoom()),
    };
    Rank {
        class,
        dist,
        detail,
    }
}

/// Classes a failure as the client's read classes it
/// (`ts/client/src/service/sys/fs/read.ts`). An object's failure comes back the same
/// however often it is asked, because within a version the key fixes the
/// bytes.
pub(crate) fn fault(failure: Failure) -> Fault {
    match failure.errnum {
        // The browser reports that it is offline.
        Errno::ENETUNREACH => Fault::Network,
        // Any status other than 204 and 404, or no response while online. The
        // Worker answers only 200, 204 and 404, so such a failure comes from
        // the platform under it: an internal error in R2, a Cloudflare
        // incident, the free plan's daily cap failing closed, or an edge
        // refusing the connection.
        Errno::EACCES | Errno::EIO => Fault::Server,
        // ENXIO is a 404: the key names nothing. EBADMSG is a body that is
        // empty or does not decode, such as one still packed in an encoding
        // this browser does not undo.
        // EFBIG is a body larger than the heap takes, or than a packet of its
        // kind can be.
        _ => Fault::Object,
    }
}

/// The wait in ms after the server's `fail_cnt`th failure in a row, or None
/// once the server is given up. `seed` differs for each failure.
///
/// The wait grows exponentially because the server is shared, so the
/// retries of every client add up on it. It follows AWS's "full jitter",
/// drawn uniformly up to the capped exponential, so clients that one outage
/// failed together do not return together.
pub(crate) fn backoff(fail_cnt: u32, seed: u64) -> Option<f64> {
    if fail_cnt > param::RETRY_ATTEMPT_CNT {
        return None;
    }
    let cap = (param::RETRY_BASE_MS * libm::exp2(f64::from(fail_cnt - 1))).min(param::RETRY_CAP_MS);
    Some(cap * u32_to_unit(SplitMix64::new(seed).next_u32()))
}

#[cfg(test)]
#[path = "policy.test.rs"]
mod test;
