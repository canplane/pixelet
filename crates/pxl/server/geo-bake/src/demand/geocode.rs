// pxl-geo-bake/demand/geocode.rs

use super::{Dataset, Failure, Pool, Source as Producer};
use crate::geocode::PlaceDict;
use pxl_spec::bake::constant::LEVEL0_ZOOM;
use pxl_spec::bake::place_page::{encode_place_group, DICT_OBJECT, GROUP_PAGES, GROUP_SHAMT};
use pxl_spec::bake::quadkey::index_of;
use pxl_spec::space::pvf3::ivec;
use std::sync::{Arc, OnceLock};

#[derive(Default)]
pub(super) struct Source {
    dict: OnceLock<Arc<PlaceDict>>,
}

pub(super) async fn resolve(data: Arc<Dataset>, pool: Arc<Pool>, name: &str) -> Result<String, Failure> {
    let group = if name == DICT_OBJECT {
        None
    } else {
        Some(index_of(name).filter(|(z, _)| *z == LEVEL0_ZOOM - GROUP_SHAMT).ok_or(Failure::BadName)?.1)
    };
    let d = data.clone();
    let p = pool.clone();
    data.cache
        .once(DICT_OBJECT.into(), async move {
            tokio::task::spawn_blocking(move || {
                let Producer::Geocode(source) = &d.source else { unreachable!() };
                let dict = p.with(|w| w.geocode.divisions(&mut w.client)).map_err(|e| e.to_string())?;
                let dict = Arc::new(PlaceDict::of(dict));
                d.cache.publish(DICT_OBJECT, Some(&dict.packet().map_err(|e| format!("{e:?}"))?))?;
                let _ = source.dict.set(dict);
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?
        })
        .await?;
    if let Some(group) = group {
        let d = data.clone();
        let name = name.to_owned();
        data.cache
            .once(name.clone(), async move {
                tokio::task::spawn_blocking(move || {
                    let Producer::Geocode(source) = &d.source else { unreachable!() };
                    let dict = source.dict.get().expect("dictionary settled");
                    let base = ivec::sh(group, GROUP_SHAMT as i32);
                    let side = 1i64 << GROUP_SHAMT;
                    let leaves: Vec<_> = (0..GROUP_PAGES as i64).map(|k| ivec::add(base, ivec::gather(k % side, k / side, 0))).collect();
                    let group = pool.with(|w| w.geocode.group(&mut w.client, dict, &leaves))?;
                    let packet = group.map(|g| encode_place_group(&g).map_err(|e| format!("{e:?}"))).transpose()?;
                    d.cache.publish(&name, packet.as_deref())
                })
                .await
                .map_err(|e| e.to_string())?
            })
            .await?;
    }
    Ok(name.into())
}
