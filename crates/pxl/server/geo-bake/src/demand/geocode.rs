// pxl-geo-bake/demand/geocode.rs

use super::job::{self, Job, Publication, Runner};
use super::{Dataset, Failure};
use crate::geocode::provider::overture::{GeocodeProvider, READS};
use crate::geocode::PlaceDict;
use crate::util::{pack, Level};
use pxl_spec::bake::place_page::{encode_place_group, Named, GROUP_PAGES};
use pxl_spec::bake::place_page::{DICT_OBJECT, GROUP_SHAMT, GROUP_KEY_DEPTH};
use pxl_spec::bake::quadkey::index_of;
use pxl_spec::place::PlaceRank;
use pxl_spec::space::pvf3::ivec;
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
        .once(DICT_OBJECT.into(), move |deadline| async move {
            r.run(
                Job::Dictionary,
                d.clone(),
                deadline,
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
            .once(name.clone(), move |deadline| async move {
                runner
                    .run(
                        Job::Geocode {
                            group,
                            dictionary: d.cache.root.join(DICT_OBJECT).join("dictionary.json"),
                        },
                        d.clone(),
                        deadline,
                        Publication::Packet(name),
                    )
                    .await
            })
            .await?;
    }
    Ok(if name == DICT_OBJECT { format!("{DICT_OBJECT}/packet") } else { name.into() })
}

// Server-only backing preserves the dictionary's identity order across jobs.
type Dictionary = Vec<(String, PlaceRank, String, Vec<(String, String)>)>;

pub(super) fn dictionary(out: &Path, seconds: u64) -> Result<(), String> {
    let out = out.join("dictionary");
    std::fs::create_dir(&out).map_err(|e| e.to_string())?;
    let (_, mut client) = job::connect(&[&READS], seconds)?;
    let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
    let rows = provider.dictionary(&mut client).map_err(|e| e.to_string())?;
    let backing: Dictionary = rows.iter().map(|(id, d)| (id.clone(), d.rank, d.name.clone(), d.names.clone())).collect();
    std::fs::write(out.join("dictionary.json"), serde_json::to_vec(&backing).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let packet = pack(&PlaceDict::of(rows).packet().map_err(|e| format!("{e:?}"))?, Level::Live).map_err(|e| e.to_string())?;
    job::write_packet(&out, Some(&packet))
}

pub(super) fn execute(group: u64, dictionary: &Path, out: &Path, seconds: u64) -> Result<(), String> {
    let rows: Dictionary = serde_json::from_slice(&std::fs::read(dictionary).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let dict = PlaceDict::of(rows.into_iter().map(|(id, rank, name, names)| (id, Named { rank, name, names })).collect());
    let (_, mut client) = job::connect(&[&READS], seconds)?;
    let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
    let base = ivec::sh(group, GROUP_SHAMT as i32);
    let side = 1i64 << GROUP_SHAMT;
    let coverage = crate::coverage::Coverage::of_source()?;
    let leaves: Vec<_> = (0..GROUP_PAGES as i64).map(|k| ivec::add(base, ivec::gather(k % side, k / side, 0))).filter(|&l| coverage.contains(l)).collect();
    let packet = provider
        .group(&mut client, &dict, &leaves)?
        .map(|g| {
            let raw = encode_place_group(&g).map_err(|e| format!("{e:?}"))?;
            pack(&raw, Level::Live).map_err(|e| e.to_string())
        })
        .transpose()?;
    job::write_packet(out, packet.as_deref())
}
