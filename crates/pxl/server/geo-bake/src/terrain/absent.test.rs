// pxl-geo-bake/terrain/absent.test.rs

use super::*;

/// A coverage whose west edge falls inside every zoom's page above the leaves, at
/// an odd page as the real one does.
fn absence() -> (Absence, Page) {
    let leaf = pxl_spec::space::pvf3::ivec::gather(11287, 3200, 0);
    (Absence::new(Coverage { x: (11287, 11400), y: (3100, 3300) }), Page::over(LEAF_Z, leaf))
}

#[test]
fn an_absent_page_is_the_fold_of_the_absent_pages_under_it() {
    let (absence, leaf) = absence();
    for z in (LEAF_Z - 10..LEAF_Z).rev() {
        let p = Page::over(z, leaf.first_leaf());
        let children = p.children().map(|c| absence.at(c));
        let folded = Terrain::fold_page(p, |q| p.children().iter().position(|&c| c == q).map(|i| children[i].as_ref())).unwrap();
        assert_eq!(absence.at(p).ground, folded.ground, "z{z}");
    }
}

#[test]
fn inside_the_coverage_absence_is_sea_and_past_it_void() {
    let (absence, leaf) = absence();
    assert_eq!(absence.at(leaf).ground.cells[0], ground::water);
    let west = Page::new(LEAF_Z, pxl_spec::space::pvf3::ivec::gather(11286, 3200, 0));
    assert_eq!(absence.at(west).ground.cells[0], sentinel::void);
}
