// pxl-render/frame/shader/sequence.test.rs

use super::*;
use crate::frame::shader::access::{
    found, role, Group, ARGS, ARGS_INDIRECT, COPY, DRAW, NO_BUF, STATE, WRITE_BY_RUNTIME,
};

/// The sources whose calls take their offsets from a step here.
const SOURCES: [(&str, &str); 8] = [
    ("ray/cast.rs", include_str!("../../ray/cast.rs")),
    ("ray/count.rs", include_str!("../../ray/count.rs")),
    ("ray/raster.rs", include_str!("../../ray/raster.rs")),
    ("generator.rs", include_str!("../../gpu/generator.rs")),
    ("shadow.rs", include_str!("../../pass/shadow.rs")),
    ("pass/decor.rs", include_str!("../../pass/decor.rs")),
    ("pass/impostor.rs", include_str!("../../pass/impostor.rs")),
    ("renderer.rs", include_str!("../../renderer.rs")),
];

/// Every render pass an indirect draw of the frame runs in is declared where the host begins
/// it. The host declares the pass with the list the draw names (`Step::in_pass`) through
/// `Gpu::declare_pass`, which refuses any other pipeline the pass sets. Every list a pass is
/// declared with is one of the frame's.
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

/// Whether `statement` names one of the frame's buffers by the host's field for it, as in
/// `.marks,` or `.light_args)`.
fn names_buf(statement: &str) -> bool {
    BUFS.iter().any(|b| {
        let field = b.name.rsplit('.').next().unwrap_or(b.name);
        [",", ")", " "]
            .iter()
            .any(|end| statement.contains(&format!(".{field}{end}")))
    })
}

/// Every indirect dispatch and draw in the sources takes the place of its count from a step,
/// and every copy and host's write of one of the frame's buffers takes its offsets from one.
/// The steps a function names come in the frame's order, except the host's writes, which land
/// first wherever they are named. Every step that reads a count, copies or writes is named by
/// some source.
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
                if names_buf(statement) {
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
        // The steps a function names, in the order of its text, are held to the frame's order.
        // A pass's list is not a step.
        let at: Vec<(usize, &str)> = text
            .match_indices("sequence::")
            .filter_map(|(k, _)| named(&text[k..]).map(|n| (k, n)))
            .filter(|(_, n)| !n.ends_with("_PASS"))
            .collect();
        let starts: Vec<usize> = text.match_indices("fn ").map(|(k, _)| k).collect();
        let by_host = |n: &str| FRAME.steps[place(n)].kind == WRITE_BY_RUNTIME;
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
        if step.indirect.buf != NO_BUF || step.kind == COPY || step.kind == WRITE_BY_RUNTIME {
            assert!(
                named_anywhere.contains(&k),
                "`{}` is a step no source calls",
                step.name
            );
        }
    }
}

/// A binding's role in a call is `ARGS_INDIRECT` for the args the call reads its count from,
/// which it binds read-only, `ARGS_WRITTEN` for args its pass writes, and otherwise its
/// buffer's role.
#[test]
fn a_binding_takes_its_role_from_its_buffer_and_its_call() {
    assert_eq!(role(&FRAME, &PX, 30), ARGS_INDIRECT);
    assert_eq!(role(&FRAME, &FACE, 16), ARGS_INDIRECT);
    assert_eq!(role(&FRAME, &REC_HITS, 9), STATE);
    let corners = FRAME.steps.iter().find(|s| s.name == "corners");
    assert_eq!(corners.map(|s| role(&FRAME, s, 9)), Some(STATE));
}

/// Rule 1 finds a dispatch off the light's args whose own group binds those args writable:
/// the light's propagation dispatched off the args it binds writable at 21, which WebGPU
/// refuses.
#[test]
fn an_indirect_call_off_args_its_group_binds_writable_is_found() {
    const ARGS_BOUND: Group = Group {
        binds: &[bind(21, 0)],
    };
    const SHIPPED: Frame = Frame {
        stages: &["the light's cast"],
        bufs: &[buf("light_args", ARGS)],
        appends: &[],
        seeds: &[],
        bounds: &[],
        persistent: &[],
        steps: &[
            Step::write("light_args", at(0, 0, light::ARGS_WORDS)),
            Step::dispatch_off(
                "light_propagate",
                0,
                COMPUTE_LIGHT_PROPAGATE,
                &ARGS_BOUND,
                (0, light::ARGS_PASSES),
            ),
        ],
    };
    assert!(found(&SHIPPED).contains(&(1, "light_propagate", "light_args")));
}
