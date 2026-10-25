// xpute-gpu/encode.test.rs

use xpute_guest::ipc::stream::{Stream, WORD};
use xpute_kit::golden::Golden;

use crate::programs::{At, Encoder, PROGRAMS};

const GOLDEN: &str = include_str!("../../../spec/golden/gpu/programs.tsv");

/// The only test here that sets the memory's base, so nothing races for it.
#[test]
fn every_program_writes_the_words_its_record_holds() {
    let g = Golden::load(GOLDEN);
    let [region, stream_words, head] = ["region", "stream.words", "stream.head"].map(|k| g.u32(k) as usize);
    let memory: &'static mut [u32] = Vec::leak(vec![0; region / 4]);
    let base = memory.as_mut_ptr().cast::<u8>();
    // SAFETY: leaked, so it lives as long as the process.
    unsafe { xpute_guest::mem::set(base.expose_provenance(), region) };
    let names: Vec<&str> = PROGRAMS.iter().map(|p| p.name).collect();
    let mut recorded = Vec::new();
    g.each("program", |k| recorded.push(g.s(k)));
    assert_eq!(names, recorded, "the programs, as the record lists them");
    for p in &PROGRAMS {
        // SAFETY: the region leaked above, which the program and the stream share as a guest's memory does.
        let bytes = unsafe { core::slice::from_raw_parts_mut(base, region) };
        bytes.fill(0);
        let key = format!("{}.input", p.name);
        let input = if g.has(&key) { g.bytes(&key) } else { Vec::new() };
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
        let mut stream = unsafe { Stream::new(xpute_guest::mem::core(), 0, stream_words, head) };
        let mut turn = 0;
        (p.encode)(&mut Encoder::new(), &mut stream, &at, &mut |s: &mut Stream| {
            s.publish();
            // SAFETY: as `bytes`; the stream is between records.
            let words = unsafe { core::slice::from_raw_parts(base.cast::<u32>(), head + xpute_guest::mem::core().word(s.head_at() + Stream::END * WORD) as usize) };
            let got: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
            assert_eq!(got, g.bytes(&format!("{}.turn.{turn}", p.name)), "{}.turn.{turn}", p.name);
            turn += 1;
        });
        assert_eq!(turn, g.len(&format!("{}.turn", p.name)), "{}'s turns", p.name);
    }
}
