use super::holds_cloud;

/// A patch's cells hold about the share asked, some patches more, some
/// fewer, and none past its 256.
#[test]
fn patches_hold_about_the_share() {
    let mut all = 0;
    let (mut least, mut most) = (256, 0);
    for p in 0..64 {
        let mut n = 0;
        for j in 0..16 {
            for i in 0..16 {
                n += holds_cloud([p * 16 + i, 7 * 16 + j], 128) as u32;
            }
        }
        all += n;
        least = least.min(n);
        most = most.max(n);
    }
    let mean = all / 64;
    assert!((112..=144).contains(&mean), "{mean}");
    assert!(least < most);
}
