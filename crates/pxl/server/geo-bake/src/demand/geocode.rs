// pxl-geo-bake/demand/geocode.rs

use super::job::{self, Job, Publication, Runner};
use super::{Dataset, Failure};
use crate::geocode::provider::overture::{GeocodeProvider, READS};
use crate::geocode::PlaceDict;
use crate::util::{pack, Level};
use pxl_spec::bake::place_tile::{encode_place_group, Named, GROUP_TILES};
use pxl_spec::bake::place_tile::{DICT_OBJECT, GROUP_KEY_DEPTH, GROUP_SHAMT};
use pxl_spec::bake::quadkey::index_of;
use pxl_spec::space::coord::pvf3::ivec;
use std::path::Path;
use std::sync::Arc;

pub(super) async fn resolve(data: Arc<Dataset>, runner: Arc<Runner>, name: &str) -> Result<String, Failure> {
    let group = if name == DICT_OBJECT {
        None
    } else {
        Some(index_of(name).filter(|(depth, _)| *depth == GROUP_KEY_DEPTH).ok_or(Failure::BadName)?.1)
    };
    let d = data.clone();
    let r = runner.clone();
    data.cache
        .once(DICT_OBJECT.into(), true, move |lease| async move {
            r.run(
                Job::Dictionary,
                d.clone(),
                lease,
                Publication::Directory {
                    source: "dictionary".into(),
                    target: DICT_OBJECT.into(),
                },
            )
            .await
        })
        .await?;
    if let Some(group) = group {
        let d = data.clone();
        let name = name.to_owned();
        data.cache
            .once(name.clone(), true, move |lease| async move {
                runner
                    .run(
                        Job::Geocode {
                            group,
                            dictionary: d.cache.root.join(DICT_OBJECT).join("dictionary.json"),
                        },
                        d.clone(),
                        lease,
                        Publication::Packet(name),
                    )
                    .await
            })
            .await?;
    }
    Ok(if name == DICT_OBJECT { format!("{DICT_OBJECT}/packet") } else { name.into() })
}

/// The dictionary's places by id, in its order, for the jobs that number labels by it.
type Dictionary = Vec<(String, Named)>;

pub(super) fn dictionary(out: &Path, seconds: u64) -> Result<(), String> {
    let out = out.join("dictionary");
    std::fs::create_dir(&out).map_err(|e| e.to_string())?;
    // The dictionary names every place, so it needs every shard.
    let (_, mut client) = job::connect(&[&READS], &crate::coverage::Coverage::of_source()?.shards().collect(), seconds)?;
    let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
    let rows = provider.dictionary(&mut client).map_err(|e| e.to_string())?;
    std::fs::write(out.join("dictionary.json"), serde_json::to_vec(&rows).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let packet = pack(&PlaceDict::of(rows).packet().map_err(|e| format!("{e:?}"))?, Level::Live).map_err(|e| e.to_string())?;
    job::write_packet(&out, Some(&packet))
}

pub(super) fn execute(group: u64, dictionary: &Path, out: &Path, seconds: u64) -> Result<(), String> {
    let rows: Dictionary = serde_json::from_slice(&std::fs::read(dictionary).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let dict = PlaceDict::of(rows);
    let base = ivec::sh(group, GROUP_SHAMT as i32);
    let side = 1i64 << GROUP_SHAMT;
    let coverage = crate::coverage::Coverage::of_source()?;
    let (_, mut client) = job::connect(&[&READS], &coverage.around(base), seconds)?;
    let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
    let leaves: Vec<_> = (0..GROUP_TILES as i64)
        .map(|k| ivec::add(base, ivec::gather(k % side, k / side, 0)))
        .filter(|&l| coverage.contains(l))
        .collect();
    let packet = provider
        .group(&mut client, &dict, &leaves)?
        .map(|g| {
            let raw = encode_place_group(&g).map_err(|e| format!("{e:?}"))?;
            pack(&raw, Level::Live).map_err(|e| e.to_string())
        })
        .transpose()?;
    job::write_packet(out, packet.as_deref())
}
