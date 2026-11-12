// pxl-core/domain/layer/earth.test.rs

use super::*;
use fnv::FnvHashMap;
use pxl_spec::bake::tile::earth::EARTH_BYTES;

fn packet() -> Body {
    Body::Heap(earth::encode(&vec![0; EARTH_BYTES]).unwrap().into_boxed_slice())
}

fn tile(x: i32) -> Quadrant {
    Quadrant::new(EARTH_ZOOM, [x, 0])
}

/// A cache with `tiles` asked for, each holding a packet the GPU does not have.
fn holding(tiles: &[Quadrant]) -> &'static mut TileCache<EarthTile> {
    EARTH.set(TileCache::default());
    let cache = EarthTile::tile_cache();
    let asks: FnvHashMap<Quadrant, ()> = tiles.iter().map(|&p| (p, ())).collect();
    cache.ask(&asks);
    for &p in tiles {
        cache
            .admit(p, |_| None, |_| Ok::<_, ()>(EarthTile::Waiting(packet())))
            .unwrap();
    }
    cache
}

fn is_sent_on(tile: Option<&EarthTile>, on: u32) -> bool {
    matches!(tile, Some(EarthTile::Sent(s)) if *s == on)
}

#[test]
fn a_full_stream_leaves_the_rest_waiting() {
    crate::on_core(|| {
        let (a, b) = (tile(0), tile(1));
        let cache = holding(&[a, b]);
        let mut stream_has_room = true;
        upload(&mut |_, _| std::mem::take(&mut stream_has_room), 7);
        assert!(
            is_sent_on(cache.get(a), 7) && matches!(cache.get(b), Some(EarthTile::Waiting(_))),
            "the first taken, the second kept for a later frame"
        );
        upload(&mut |_, _| true, 7);
        assert!(is_sent_on(cache.get(b), 7));
        EARTH.set(TileCache::default());
    });
}

#[test]
fn a_new_device_fetches_again() {
    crate::on_core(|| {
        let a = tile(0);
        let cache = holding(&[a]);
        upload(&mut |_, _| true, 3);
        assert!(!cache.needs_refetch(a));
        upload(&mut |_, _| false, 4);
        assert!(
            cache.needs_refetch(a) && is_sent_on(cache.get(a), 3),
            "its texels went to device 3, so it is fetched again and stays until they come"
        );
        EARTH.set(TileCache::default());
    });
}
