// pxl-render/gpu/ray/sequence.test.rs

use super::*;
use crate::frame::shader::access::{found, role, ARGS_INDIRECT, ARGS_WRITTEN, NO_BUFFER};

/// The sources whose indirect calls take their count's place from a step here.
const SOURCES: [(&str, &str); 4] = [
    ("ray/cast.rs", include_str!("cast.rs")),
    ("ray/raster.rs", include_str!("raster.rs")),
    ("generator.rs", include_str!("../generator.rs")),
    ("shadow.rs", include_str!("../shadow.rs")),
];

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

/// Every indirect dispatch and draw of the sources takes its count's place from a step, and
/// those a function names come in the frame's order.
#[test]
fn the_host_calls_indirect_off_the_steps_in_their_order() {
    let order: Vec<&str> = FRAME.steps.iter().map(|s| s.name).collect();
    let place = |name: &str| {
        order
            .iter()
            .position(|n| n.to_uppercase() == name)
            .unwrap_or_else(|| panic!("`sequence::{name}` names no step of the frame"))
    };
    let mut named_anywhere = Vec::new();
    for (file, text) in SOURCES {
        for call in ["dispatch_indirect(", "draw_indirect("] {
            for (_, statement) in calls(text, call) {
                assert!(
                    statement.contains(".at()"),
                    "{file}: an indirect call off no step: {statement}"
                );
            }
        }
        // The steps a function names, in its text's order, against the frame's.
        let at: Vec<(usize, &str)> = text
            .match_indices("sequence::")
            .filter_map(|(k, _)| named(&text[k..]).map(|n| (k, n)))
            .collect();
        let starts: Vec<usize> = text.match_indices("fn ").map(|(k, _)| k).collect();
        for w in at.windows(2) {
            let ((a, first), (b, second)) = (w[0], w[1]);
            let same_fn = !starts.iter().any(|&s| a < s && s < b);
            if same_fn && first != second {
                assert!(
                    place(first) < place(second),
                    "{file}: `{first}` called before `{second}`, the frame runs them the other way"
                );
            }
        }
        named_anywhere.extend(at.iter().map(|(_, n)| place(n)));
    }
    // Every indirect call of the frame is one the sources make.
    for (k, step) in FRAME.steps.iter().enumerate() {
        if step.indirect.buf != NO_BUFFER {
            assert!(
                named_anywhere.contains(&k),
                "`{}` reads its count off a step no source calls",
                step.name
            );
        }
    }
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

/// A binding's role in a call: the args it reads its count from, bound read only; args its
/// pass writes; else its buffer's.
#[test]
fn a_binding_takes_its_role_from_its_buffer_and_its_call() {
    assert_eq!(role(&FRAME, &PX, 30), ARGS_INDIRECT);
    assert_eq!(role(&FRAME, &FACE, 16), ARGS_INDIRECT);
    assert_eq!(role(&FRAME, &LIGHT_CLAIM, 21), ARGS_WRITTEN);
    assert_eq!(role(&FRAME, &STOPS, 9), ARGS_INDIRECT);
    assert_eq!(role(&FRAME, &BEAM, 9), STATE);
}
