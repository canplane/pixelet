// pxl-render/gpu/ray/sequence.test.rs

use super::*;
use crate::frame::shader::access::{
    found, role, ARGS_INDIRECT, ARGS_WRITTEN, COPY, DRAW, NO_BUFFER, WRITE_BY_HOST,
};

/// The sources whose calls take their offsets from a step here.
const SOURCES: [(&str, &str); 8] = [
    ("ray/cast.rs", include_str!("cast.rs")),
    ("ray/count.rs", include_str!("count.rs")),
    ("ray/raster.rs", include_str!("raster.rs")),
    ("generator.rs", include_str!("../generator.rs")),
    ("shadow.rs", include_str!("../shadow.rs")),
    ("pass/decor.rs", include_str!("../pass/decor.rs")),
    ("pass/impostor.rs", include_str!("../pass/impostor.rs")),
    ("renderer.rs", include_str!("../renderer.rs")),
];

/// Every render pass a draw of the frame's runs in declared where the host begins it: each
/// list a draw names (`Step::in_pass`) held to a pass by `Gpu::declare_pass`, which refuses
/// any other pipeline the pass sets; and every pass declared so holds a list of the frame's.
#[test]
fn the_host_declares_each_render_pass_of_an_indirect_draw() {
    let lists = [
        ("RASTER_PASS", RASTER_PASS),
        ("LIGHT_RASTER_PASS", LIGHT_RASTER_PASS),
        ("MAP_PASS", MAP_PASS),
        ("LIT_PASS", LIT_PASS),
    ];
    let declared: Vec<&str> = SOURCES
        .iter()
        .flat_map(|(file, text)| {
            calls(text, "declare_pass(").into_iter().map(move |(_, s)| {
                named(s).unwrap_or_else(|| panic!("{file}: a pass declared off no list: {s}"))
            })
        })
        .collect();
    for step in FRAME.steps.iter().filter(|s| s.kind == DRAW) {
        let (name, _) = lists
            .iter()
            .find(|(_, list)| core::ptr::eq(*list, step.pass))
            .unwrap_or_else(|| panic!("`{}` draws in a pass of no list here", step.name));
        assert!(
            declared.contains(name),
            "`{}` draws in `{name}`, which no source declares where it begins the pass",
            step.name
        );
    }
    for name in &declared {
        assert!(
            lists.iter().any(|(n, _)| n == name),
            "a pass declared off `{name}`, no list a draw names"
        );
    }
}

/// The hazards the frame holds, each as the build names it in `KNOWN`.
#[test]
fn the_frame_holds_the_hazards_known_names() {
    let found = found(&FRAME);
    let listed: Vec<String> = found
        .iter()
        .map(|(rule, step, with)| format!("rule {rule}: {step} with {with}"))
        .collect();
    assert_eq!(
        found.len(),
        KNOWN.len(),
        "the frame's hazards:\n{}",
        listed.join("\n")
    );
}

/// Each statement of `text` that calls `call`, from the call to its `;`.
fn calls<'a>(text: &'a str, call: &str) -> Vec<(usize, &'a str)> {
    text.match_indices(call)
        .filter(|(at, _)| !text[..*at].ends_with("fn "))
        .map(|(at, _)| {
            let end = text[at..].find(';').map_or(text.len(), |e| at + e);
            (at, &text[at..end])
        })
        .collect()
}

/// The step a text names first, by its constant's name.
fn named(statement: &str) -> Option<&str> {
    let at = statement.find("sequence::")? + "sequence::".len();
    let name = &statement[at..];
    let end = name
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(name.len());
    Some(&name[..end])
}

/// Whether `statement` names one of the frame's buffers by the host's field: `.marks,`,
/// `.light_args)`.
fn names_buffer(statement: &str) -> bool {
    BUFFERS.iter().any(|b| {
        let field = b.name.rsplit('.').next().unwrap_or(b.name);
        [",", ")", " "]
            .iter()
            .any(|end| statement.contains(&format!(".{field}{end}")))
    })
}

/// Every indirect dispatch and draw of the sources takes its count's place from a step, and
/// every copy and host's write of one of the frame's buffers its offsets; the steps a function
/// names come in the frame's order, the host's writes, which land first, apart; and every
/// step that reads a count, copies or writes is named by a source.
#[test]
fn the_host_calls_off_the_steps_in_their_order() {
    let order: Vec<&str> = FRAME.steps.iter().map(|s| s.name).collect();
    let place = |name: &str| {
        order
            .iter()
            .position(|n| n.to_uppercase() == name)
            .unwrap_or_else(|| panic!("`sequence::{name}` names no step of the frame"))
    };
    let mut named_anywhere = Vec::new();
    for (file, text) in SOURCES {
        for call in [
            "dispatch_indirect(",
            "draw_indirect(",
            "draw_indexed_indirect(",
        ] {
            for (_, statement) in calls(text, call) {
                assert!(
                    statement.contains(".at()") || statement.contains(".off("),
                    "{file}: an indirect call off no step: {statement}"
                );
            }
        }
        for call in ["copy_buf(", "write_buf("] {
            for (_, statement) in calls(text, call) {
                if names_buffer(statement) {
                    assert!(
                        statement.contains("sequence::")
                            || statement.contains("from_at()")
                            || statement.contains("to_at()"),
                        "{file}: a copy or a write of a buffer the frame shares off no step: \
                         {statement}"
                    );
                }
            }
        }
        // The steps a function names, in its text's order, against the frame's; a pass's list
        // is no step.
        let at: Vec<(usize, &str)> = text
            .match_indices("sequence::")
            .filter_map(|(k, _)| named(&text[k..]).map(|n| (k, n)))
            .filter(|(_, n)| !n.ends_with("_PASS"))
            .collect();
        let starts: Vec<usize> = text.match_indices("fn ").map(|(k, _)| k).collect();
        let by_host = |n: &str| FRAME.steps[place(n)].kind == WRITE_BY_HOST;
        for w in at.windows(2) {
            let ((a, first), (b, second)) = (w[0], w[1]);
            let same_fn = !starts.iter().any(|&s| a < s && s < b);
            if same_fn && first != second && !by_host(first) && !by_host(second) {
                assert!(
                    place(first) < place(second),
                    "{file}: `{first}` called before `{second}`, the frame runs them the other way"
                );
            }
        }
        named_anywhere.extend(at.iter().map(|(_, n)| place(n)));
    }
    for (k, step) in FRAME.steps.iter().enumerate() {
        if step.indirect.buf != NO_BUFFER || step.kind == COPY || step.kind == WRITE_BY_HOST {
            assert!(
                named_anywhere.contains(&k),
                "`{}` is a step no source calls",
                step.name
            );
        }
    }
}

/// A binding's role in a call: the args it reads its count from, bound read only; args its
/// pass writes; else its buffer's.
#[test]
fn a_binding_takes_its_role_from_its_buffer_and_its_call() {
    assert_eq!(role(&FRAME, &PX, 30), ARGS_INDIRECT);
    assert_eq!(role(&FRAME, &FACE, 16), ARGS_INDIRECT);
    assert_eq!(role(&FRAME, &LIGHT_CLAIM, 21), ARGS_WRITTEN);
    assert_eq!(role(&FRAME, &STOPS, 9), STATE);
    assert_eq!(role(&FRAME, &BEAM, 9), STATE);
}

/// The call that shipped refused: the claim off the light's args, which the propagation's
/// group, set for it, binds writable (034ddad063).
#[test]
fn an_indirect_call_off_args_its_group_binds_writable_is_found() {
    const ARGS_BOUND: Group = Group {
        layout: &LIGHT_PROPAGATE_GROUP,
        binds: &[bind(21, 0)],
    };
    const SHIPPED: Frame = Frame {
        stages: &["the light's cast"],
        buffers: &[buffer("light_args", ARGS)],
        appends: &[],
        seeds: &[],
        bounds: &[],
        kept: &[],
        steps: &[
            Step::write("light_args", at(0, 0, light::ARGS_WORDS)),
            Step::dispatch_off(
                "light_claim",
                0,
                COMPUTE_LIGHT_CLAIM,
                &[&ARGS_BOUND],
                (0, light::ARGS_PASSES),
            ),
        ],
    };
    assert!(found(&SHIPPED).contains(&(1, "light_claim", "light_args")));
}
