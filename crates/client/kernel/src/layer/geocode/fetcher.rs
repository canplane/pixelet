// pxl-kernel/layer/geocode/fetcher.rs

//! Demands the dictionary, once a session, and the group the asker's center
//! is in. A key with any answer is not asked again. Both sit behind every
//! terrain tile: a label must not take a credit from the ground.

use pxl_spec::bake::division_tile::GROUP_SHAMT;
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::coord::pvf3::Pvf3;

use crate::io::demand::{Ask, Category};
use crate::io::fetch::{answer_settled, next_answered, release, Answered};
use crate::io::request::{attempt_of, declare};
use crate::layer::geocode::store;
use pxl_spec::space::coord::zoom::idx;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

const DICT_SUBJECT: u64 = 0;

/// The group a point is in.
pub fn group_of(at: &Pvf3) -> IVec {
    ivec::sh(idx(LEAF_ZOOM, at.i), -(GROUP_SHAMT as i32))
}

/// Takes what arrived, then declares. Declared every turn: an unchanged set
/// changes nothing, so nothing is remembered between turns.
pub fn geocode_fetcher_step(center: Option<IVec>) {
    while let Some(Answered {
        key,
        category,
        subject,
        block,
        phases,
    }) = next_answered(&[Category::geocode, Category::geocode_dict])
    {
        // A key the request scheduler no longer holds is one a reset forgot.
        if attempt_of(key).is_none() {
            release(block);
            continue;
        }
        let result = match (category, block) {
            (Category::geocode, None) => {
                store::absent_group(subject);
                Ok(())
            }
            // A bake with no dictionary names nothing: an answer, so not asked again.
            (_, None) => {
                store::absent_dict();
                Ok(())
            }
            // SAFETY (both): the heap block given for this body alone; the store
            // keeps it, or frees it if it does not open.
            (Category::geocode, Some(b)) => unsafe { store::adopt_group(subject, b) }.map_err(Into::into),
            (_, Some(b)) => unsafe { store::adopt_dict(b) }.map_err(Into::into),
        };
        answer_settled(key, result, phases);
    }
    let dict = [(DICT_SUBJECT, Ask::PLAIN)];
    declare(Category::geocode_dict, if store::dict_answered() { &[] } else { &dict });

    match center {
        Some(idx) if store::answered() != Some(idx) => declare(Category::geocode, &[(idx, Ask::PLAIN)]),
        _ => declare(Category::geocode, &[]),
    }
}
