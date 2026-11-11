// xpute-gpu/encode.test.rs

use xpute_core::io::stream::{Stream, WORD};
use xpute_golden::Golden;

use crate::program::{At, Encoder, PROGRAMS};

const GOLDEN: &str = include_str!("../../../spec/golden/gpu/program.tsv");

/// The memory's base is global, and this is the only test that sets it, so
/// no other test races for it.
#[test]
fn programs_match_record() {
    let g = Golden::load(GOLDEN);
    let [region, stream_words, head] =
        ["region", "stream.words", "stream.head"].map(|k| g.u32(k) as usize);
    let mem: &'static mut [u32] = Vec::leak(vec![0; region / 4]);
    let base = mem.as_mut_ptr().cast::<u8>();
    // SAFETY: leaked, so it lives as long as the process.
    unsafe { xpute_core::mem::set(base.expose_provenance(), region) };
    let names: Vec<&str> = PROGRAMS.iter().map(|p| p.name).collect();
    let mut recorded = Vec::new();
    g.each("program", |k| recorded.push(g.s(k)));
    assert_eq!(names, recorded, "the programs, as the record lists them");
    for p in &PROGRAMS {
        // SAFETY: the region leaked above, which the program and the stream share as a core's memory does.
        let bytes = unsafe { core::slice::from_raw_parts_mut(base, region) };
        bytes.fill(0);
        let key = format!("{}.input", p.name);
        let input = if g.has(&key) {
            g.bytes(&key)
        } else {
            Vec::new()
        };
        let input_at = g.u32("input.at") as usize;
        bytes[input_at..input_at + input.len()].copy_from_slice(&input);
        let at = At {
            base,
            input: input_at,
            input_len: input.len(),
            done: g.u32("done.at") as usize,
            readback: g.u32("readback.at") as usize,
        };
        // SAFETY: as `bytes`.
        let mut stream = unsafe { Stream::new(xpute_core::mem::core(), 0, stream_words, head) };
        let mut turn = 0;
        (p.encode)(
            &mut Encoder::new(),
            &mut stream,
            &at,
            &mut |s: &mut Stream| {
                s.publish();
                // SAFETY: as `bytes`; the stream is between records.
                let words = unsafe {
                    core::slice::from_raw_parts(
                        base.cast::<u32>(),
                        head + xpute_core::mem::core().word(s.at() + Stream::END * WORD) as usize,
                    )
                };
                let got: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
                assert_eq!(
                    got,
                    g.bytes(&format!("{}.turn.{turn}", p.name)),
                    "{}.turn.{turn}",
                    p.name
                );
                // Handed back, as a runtime hands it back once it has run it.
                xpute_core::mem::core().set_word(s.at() + Stream::END * WORD, 0);
                turn += 1;
            },
        );
        assert_eq!(
            turn,
            g.len(&format!("{}.turn", p.name)),
            "{}'s turns",
            p.name
        );
    }
}
