// pxl-spec/bake/named.rs

//! A place's names as every bake stores them, whatever the source: its own
//! name, and `(tag, name)` pairs in the languages it gives. Three leaves.

use xpute_kit::status::error::MarshalError;

use crate::bake::malformed;
use xpute_kit::wire::xtp::{BranchCursor, BranchViewImpl, NodeView, Strs};

pub struct NameLanes<'a> {
    /// Where each place's pairs start; one longer than the places.
    starts: Vec<u32>,
    names: Vec<&'a str>,
    langs: Vec<&'a str>,
}

impl<'a> NameLanes<'a> {
    /// Each place's name and its (tag, name) pairs.
    pub fn of<L: IntoIterator<Item = (&'a str, &'a str)>>(
        places: impl IntoIterator<Item = (&'a str, L)>,
    ) -> NameLanes<'a> {
        let mut lanes = NameLanes {
            starts: vec![0],
            names: Vec::new(),
            langs: Vec::new(),
        };
        for (name, names) in places {
            lanes.names.push(name);
            lanes
                .langs
                .extend(names.into_iter().flat_map(|(tag, name)| [tag, name]));
            lanes.starts.push((lanes.langs.len() / 2) as u32);
        }
        lanes
    }

    pub fn write(&'a self, b: &mut BranchViewImpl<'a>) {
        b.u32_array(Some(&self.starts))
            .strs(Some(&self.names))
            .strs(Some(&self.langs));
    }
}

/// Read where it lies.
#[derive(Clone, Copy)]
pub struct NamesView<'a> {
    starts: &'a [u32],
    names: Strs<'a>,
    langs: Strs<'a>,
}

impl<'a> NamesView<'a> {
    /// `len` places' names from leaf `at`.
    pub fn read(
        root: &BranchCursor<'a>,
        at: u32,
        len: usize,
    ) -> Result<NamesView<'a>, MarshalError> {
        let view = NamesView {
            starts: root.at(at)?.get_array::<u32>()?,
            names: root.at(at + 1)?.get_strs()?,
            langs: root.at(at + 2)?.get_strs()?,
        };
        let pairs = view.langs.len() as usize;
        let ordered =
            view.starts.first() == Some(&0) && view.starts.windows(2).all(|w| w[0] <= w[1]);
        if view.starts.len() != len + 1
            || view.names.len() as usize != len
            || !ordered
            || !pairs.is_multiple_of(2)
            || view.starts[len] as usize != pairs / 2
        {
            return Err(malformed());
        }
        Ok(view)
    }

    pub fn name(&self, i: u32) -> Option<&'a str> {
        self.names.get(i)
    }

    pub fn languages(&self, i: u32) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        let (langs, range) = (
            self.langs,
            self.starts.get(i as usize).copied().unwrap_or(0)
                ..self.starts.get(i as usize + 1).copied().unwrap_or(0),
        );
        range.filter_map(move |p| Some((langs.get(p * 2)?, langs.get(p * 2 + 1)?)))
    }
}
