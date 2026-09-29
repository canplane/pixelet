// xpute-conformance/trace.test.rs

//! The traces run against the Rust guest, by a host that knows only what the
//! contract lets it: the directory's offset, and then what the directory
//! names. It sends what a step says, rings, runs the stream and reads the
//! completion ring dry after every turn, as rule 5 asks of a host, and holds
//! what it saw to what the trace says it must see.
//!
//! Two rules are not in any trace because the host here is what keeps them:
//! the memory never moves (rule 1), and the host never runs while a turn
//! does (rule 2's exclusion). A trace holds what a guest could get wrong.

use std::cell::Cell;
use std::collections::HashMap;

use xpute_core::golden::Golden;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeView};
use xpute_runtime::abi::handle::handle_slot;
use xpute_runtime::clock::set_clock;
use xpute_runtime::ipc::directory::spec as directory;
use xpute_runtime::ipc::frame::FrameFlag;
use xpute_runtime::ipc::ring::Ring;

use crate::guest::Guest;
use crate::spec;

/// Turns a `settle` rings at most before the trace is taken to have stalled.
const MOST_SETTLE_TURNS: usize = 64;

thread_local! {
    /// The host's clock, which a `clock` step sets: a trace's times are its
    /// inputs, and only what a guest makes of them is held to anything.
    static CLOCK_MS: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    CLOCK_MS.with(Cell::get)
}

struct Completion {
    tag: u32,
    cmd: u32,
    reply: bool,
    result: i32,
}

/// What one turn left, each place on its own.
struct Turn {
    completions: Vec<Completion>,
    stream: Vec<u32>,
    wake: f64,
}

struct Host {
    _mem: Box<[u64]>,
    guest: Guest,
    /// The directory's word kept at 0, and its entries by key.
    reserved: u32,
    entries: HashMap<u32, u32>,
    sq: Ring,
    cq: Ring,
    stream: *const u32,
}

impl Host {
    /// A fresh guest over a fresh memory, found through its directory.
    fn boot() -> Host {
        let mut mem = vec![0u64; spec::MEMORY_BYTES as usize / 8].into_boxed_slice();
        let base = mem.as_mut_ptr().cast::<u8>();
        // SAFETY: the memory is the guest's size, 8-aligned, and lives in the
        // host beside the guest.
        let guest = unsafe { Guest::boot(base) };
        let (reserved, entries) = read_directory(base);
        // SAFETY: the rings and the stream are where the guest's directory
        // says, in the same memory.
        let (sq, cq, stream) = unsafe {
            (
                Ring::new(base, entries[&directory::key::SUBMISSION]),
                Ring::new(base, entries[&directory::key::COMPLETION]),
                base.add(entries[&spec::key::STREAM] as usize).cast::<u32>().cast_const(),
            )
        };
        set_clock(clock);
        Host {
            _mem: mem,
            guest,
            reserved,
            entries,
            sq,
            cq,
            stream,
        }
    }

    fn submit(&self, tag: u32, cmd: u32, ack: bool, arg: Option<u32>) {
        let packet = arg.map(|n| {
            let mut t: TreeView<'_> = TreeView::new();
            t.branch(Some(&mut |b| {
                b.u32(Some(n));
            }));
            encoder().encode(&t, TreeEncoderOptions::default()).unwrap()
        });
        let flags = if ack { FrameFlag::ACKREQ as u32 } else { 0 };
        assert_eq!(self.sq.push(tag, cmd, flags, packet.as_deref(), 0), xpute_core::status::errno::Errno::OK);
    }

    /// A turn, and everything it left taken before anything else.
    fn ring(&mut self, quota_ms: f64) -> Turn {
        let wake = self.guest.interrupt(quota_ms);
        // SAFETY: the stream's words, END first, which the turn has ended.
        let stream = unsafe {
            let end = *self.stream as usize;
            let words = core::slice::from_raw_parts(self.stream.add(1), end);
            let mut ops = Vec::new();
            let mut at = 0;
            while at < end {
                ops.push(words[at]);
                at += 2 + words[at + 1] as usize;
            }
            ops
        };
        let mut completions = Vec::new();
        while self.cq.peek() >= 0 {
            let at = self.cq.peek() as u32;
            completions.push(Completion {
                tag: self.cq.tag(at),
                cmd: self.cq.cmd(at),
                reply: self.cq.flags(at) & FrameFlag::RES as u32 != 0,
                result: self.cq.result(at),
            });
            self.cq.advance();
        }
        Turn { completions, stream, wake }
    }
}

/// The directory's reserved word and its entries, its head checked: the one
/// thing the host is told is where it is.
fn read_directory(base: *mut u8) -> (u32, HashMap<u32, u32>) {
    // SAFETY: the directory the guest wrote at the offset both are built with.
    let head = unsafe { core::slice::from_raw_parts(base.add(spec::DIRECTORY_AT as usize).cast::<u32>(), directory::HEAD as usize) };
    assert_eq!([head[0], head[1]], [directory::MAGIC, directory::VERSION], "C0: the directory's head");
    // SAFETY: as above, the entries past the head.
    let words = unsafe { core::slice::from_raw_parts(base.add(spec::DIRECTORY_AT as usize).cast::<u32>(), directory::HEAD as usize + 2 * head[2] as usize) };
    let mut entries = HashMap::new();
    for pair in words[directory::HEAD as usize..].chunks(2) {
        assert!(entries.insert(pair[0], pair[1]).is_none(), "C0: a key written twice");
    }
    (head[3], entries)
}

/// A directory key a trace names.
fn key_number(name: &str) -> u32 {
    match name {
        "SUBMISSION" => directory::key::SUBMISSION,
        "COMPLETION" => directory::key::COMPLETION,
        "STREAM" => spec::key::STREAM,
        other => panic!("no key {other}"),
    }
}

fn cmd_number(name: &str) -> u32 {
    match name {
        "OK" => spec::cmd::OK,
        "FAIL" => spec::cmd::FAIL,
        "RAISE" => spec::cmd::RAISE,
        "RECORD" => spec::cmd::RECORD,
        "SIGNAL" => spec::cmd::SIGNAL,
        "ISSUE" => spec::cmd::ISSUE,
        "RELEASE" => spec::cmd::RELEASE,
        "USE" => spec::cmd::USE,
        "MARK" => spec::cmd::MARK,
        "SINCE" => spec::cmd::SINCE,
        hex => u32::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or_else(|_| panic!("no command {hex}")),
    }
}

fn cmd_name(cmd: u32) -> String {
    match cmd {
        spec::cmd::OK => "OK".into(),
        spec::cmd::FAIL => "FAIL".into(),
        spec::cmd::RAISE => "RAISE".into(),
        spec::cmd::RECORD => "RECORD".into(),
        spec::cmd::SIGNAL => "SIGNAL".into(),
        spec::cmd::ISSUE => "ISSUE".into(),
        spec::cmd::RELEASE => "RELEASE".into(),
        spec::cmd::USE => "USE".into(),
        spec::cmd::MARK => "MARK".into(),
        spec::cmd::SINCE => "SINCE".into(),
        n => format!("0x{n:04x}"),
    }
}

/// A completion ring's sequence as a trace writes it: a reply TAG:CMD=RESULT,
/// a signal TAG:CMD, and a run of one command's signals tagged 0 up CMD*n.
fn render_completions<'a>(completions: impl Iterator<Item = &'a Completion>) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut run: Option<(u32, u32)> = None;
    let flush = |run: &mut Option<(u32, u32)>, out: &mut Vec<String>| {
        if let Some((cmd, n)) = run.take() {
            out.push(format!("{}*{n}", cmd_name(cmd)));
        }
    };
    for c in completions {
        if !c.reply {
            match run {
                Some((cmd, n)) if cmd == c.cmd && c.tag == n => run = Some((cmd, n + 1)),
                _ => {
                    flush(&mut run, &mut out);
                    if c.tag == 0 {
                        run = Some((c.cmd, 1));
                    } else {
                        out.push(format!("{}:{}", c.tag, cmd_name(c.cmd)));
                    }
                }
            }
            continue;
        }
        flush(&mut run, &mut out);
        out.push(format!("{}:{}={}", c.tag, cmd_name(c.cmd), c.result));
    }
    flush(&mut run, &mut out);
    if out.is_empty() {
        "-".into()
    } else {
        out.join(" ")
    }
}

fn render_stream<'a>(ops: impl Iterator<Item = &'a u32>) -> String {
    let ops: Vec<String> = ops.map(u32::to_string).collect();
    if ops.is_empty() {
        "-".into()
    } else {
        ops.join(" ")
    }
}

/// Holds what was seen to what a trace says, token by token. A result written
/// `$n` names a value the trace does not fix — a handle — and binds it the
/// first time, so a later step or relation can use it; after that it must be
/// the same.
fn expect(seen: &str, said: &str, vars: &mut HashMap<String, i64>, what: &str) {
    let seen_tokens: Vec<&str> = seen.split(' ').collect();
    let said_tokens: Vec<&str> = said.split(' ').collect();
    assert_eq!(seen_tokens.len(), said_tokens.len(), "{what}: saw {seen}, the trace says {said}");
    for (seen_token, said_token) in seen_tokens.iter().zip(&said_tokens) {
        match (seen_token.split_once('='), said_token.split_once('=')) {
            (Some((seen_head, value)), Some((said_head, var))) if var.starts_with('$') => {
                assert_eq!(seen_head, said_head, "{what}: saw {seen}, the trace says {said}");
                let value: i64 = value.parse().unwrap();
                assert_eq!(*vars.entry(var.to_string()).or_insert(value), value, "{what}: {var} again, and not what it was");
            }
            _ => assert_eq!(seen_token, said_token, "{what}: saw {seen}, the trace says {said}"),
        }
    }
}

/// A number a trace writes: a literal, `$n`, or `slot $n`, the slot a handle
/// names.
fn value(term: &str, vars: &HashMap<String, i64>) -> i64 {
    let var = |v: &str| *vars.get(v).unwrap_or_else(|| panic!("{v} is bound by nothing before it"));
    match term.split_once(' ') {
        Some(("slot", v)) => i64::from(handle_slot(var(v) as u32)),
        _ if term.starts_with('$') => var(term),
        _ => term.parse().unwrap(),
    }
}

/// A relation a trace says holds between what was seen: `A == B` or `A != B`.
fn relation(said: &str, vars: &HashMap<String, i64>, what: &str) {
    if let Some((a, b)) = said.split_once(" == ") {
        assert_eq!(value(a, vars), value(b, vars), "{what}: {said}");
    } else if let Some((a, b)) = said.split_once(" != ") {
        assert_ne!(value(a, vars), value(b, vars), "{what}: {said}");
    } else {
        panic!("{what}: no relation in {said}");
    }
}

fn wake_name(wake: f64) -> &'static str {
    if wake == 0.0 {
        "next"
    } else if wake > 0.0 {
        "later"
    } else {
        "none"
    }
}

/// Runs one scenario's steps on a fresh guest and holds every turn, and the
/// whole, to what the trace says of them.
fn run(g: &Golden, name: &str) {
    CLOCK_MS.with(|c| c.set(0.0));
    let mut host = Host::boot();
    let mut turns: Vec<Turn> = Vec::new();
    let mut vars: HashMap<String, i64> = HashMap::new();
    let mut i = 0;
    while g.has(&format!("{name}.step.{i}")) {
        let step: Vec<&str> = g.s(&format!("{name}.step.{i}")).split(' ').collect();
        match step.as_slice() {
            ["submit", tag, cmd, ack, rest @ ..] => {
                host.submit(tag.parse().unwrap(), cmd_number(cmd), *ack == "ack", rest.first().map(|n| value(n, &vars) as u32));
            }
            ["ring", quota] => {
                // A turn is held to the trace as soon as it is seen, so the
                // results it binds can be named by the steps after it.
                let turn = host.ring(quota.parse().unwrap());
                let key = format!("{name}.turn.{}.completions", turns.len());
                if g.has(&key) {
                    expect(&render_completions(turn.completions.iter()), g.s(&key), &mut vars, &format!("{name} turn {}", turns.len()));
                }
                turns.push(turn);
            }
            ["clock", ms] => CLOCK_MS.with(|c| c.set(ms.parse().unwrap())),
            ["settle"] => loop {
                turns.push(host.ring(0.0));
                if host.sq.is_empty() {
                    break;
                }
                assert!(turns.len() < MOST_SETTLE_TURNS, "{name}: the guest stopped applying what waits");
            },
            other => panic!("{name}: no step {other:?}"),
        }
        i += 1;
    }
    for (t, turn) in turns.iter().enumerate() {
        let at = |what: &str| format!("{name}.turn.{t}.{what}");
        if g.has(&at("stream")) {
            assert_eq!(render_stream(turn.stream.iter()), g.s(&at("stream")), "{name} turn {t}: the stream");
        }
        if g.has(&at("wake")) {
            assert_eq!(wake_name(turn.wake), g.s(&at("wake")), "{name} turn {t}: what it asked for next");
        }
    }
    let at = |what: &str| format!("{name}.{what}");
    if g.has(&at("completions")) {
        assert_eq!(render_completions(turns.iter().flat_map(|t| &t.completions)), g.s(&at("completions")), "{name}: the completion ring");
    }
    if g.has(&at("stream")) {
        assert_eq!(render_stream(turns.iter().flat_map(|t| &t.stream)), g.s(&at("stream")), "{name}: the stream");
    }
    if g.has(&at("turns.least")) {
        assert!(turns.len() >= g.u32(&at("turns.least")) as usize, "{name}: {} turns", turns.len());
    }
    if g.has(&at("wake.before_last")) {
        for (t, turn) in turns[..turns.len() - 1].iter().enumerate() {
            assert_eq!(wake_name(turn.wake), g.s(&at("wake.before_last")), "{name} turn {t}: what it asked for next");
        }
    }
    if g.has(&at("wake.last")) {
        assert_eq!(wake_name(turns.last().unwrap().wake), g.s(&at("wake.last")), "{name}: what the last turn asked for");
    }
    let mut r = 0;
    while g.has(&at(&format!("relation.{r}"))) {
        relation(g.s(&at(&format!("relation.{r}"))), &vars, name);
        r += 1;
    }
    if g.has(&at("directory.keys")) {
        for k in g.s(&at("directory.keys")).split(' ') {
            assert!(host.entries.contains_key(&key_number(k)), "{name}: the directory names no {k}");
        }
    }
    if g.has(&at("directory.reserved")) {
        assert_eq!(host.reserved, g.u32(&at("directory.reserved")), "{name}: the directory's reserved word");
    }
}

fn run_all(g: &Golden) {
    for name in g.s("scenarios").split(' ') {
        run(g, name);
    }
}

#[test]
fn trace_directory() {
    run_all(&Golden::load(include_str!("../golden/directory.tsv")));
}

#[test]
fn trace_door() {
    run_all(&Golden::load(include_str!("../golden/door.tsv")));
}

#[test]
fn trace_handle() {
    run_all(&Golden::load(include_str!("../golden/handle.tsv")));
}

#[test]
fn trace_clock() {
    run_all(&Golden::load(include_str!("../golden/clock.tsv")));
}
