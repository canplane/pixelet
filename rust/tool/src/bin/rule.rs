// pxl-tool/bin/rule.rs

//! Holds the tree to what the code must keep but a reader cannot see. It refuses raw pointer
//! casts in the crates that address memory by the link's address; any rounding other than half to even
//! where clippy does not see it; any CommonJS file; and any name, in the layers' orders, of
//! what stands above the naming file: across a crate's folders and the client's, and inside
//! the client's UI and services, the core's heads, pxl-render's, the camera's and the view's,
//! and across xpute's parts. It
//! also holds that only the top names the profiler, that no renderer stands under the core's
//! sys/, that only the screen reads the screen's pixels, that no Rust file lies outside
//! a crate's module tree, that no host of xpute's names its runtime's entries, that the two
//! workspaces patch crates.io alike, and that the modules' and files' dependencies form no
//! cycle. A rule Rust alone can keep is clippy's (rust/clippy.toml,
//! xpute/clippy.toml).
//!
//!   cargo check-rules            (in rust/; make check-rules from the root)

use std::process::{Command, ExitCode};

use pxl_tool::root;
use regex::Regex;

/// The crates that address the core's memory by the link's address, a u64, in which an address
/// becomes a pointer only through `xpute_link::mem`.
const ADDR_CRATES: [&str; 5] = [
    "xpute/link/rust/src/",
    "xpute/guest/rust/src/",
    "rust/client/core/src/",
    "rust/client/runtime/src/",
    "rust/client/render/src/",
];

/// One rounding, half to even (xpute ARCHITECTURE.md, `math/scalar`). A call that rounds
/// otherwise, in TypeScript, WGSL or C++, or a floor of a half added in any of them or in
/// Rust, is refused in pixelet's code and xpute's; Rust's rounding calls are clippy's to
/// refuse (`disallowed-methods`). xpute's scalar keeps its named variants, round_half_up and
/// round_half_away, as options a library offers.
const ROUNDING_ROOTS: [&str; 3] = ["rust/", "ts/", "xpute/"];
const ROUNDING_VARIANT_FILES: [&str; 4] = [
    "xpute/kit/ts/src/math/scalar.ts",
    "xpute/kit/ts/src/math/scalar.test.ts",
    "xpute/cpp/xpute/kit/math/scalar.hpp",
    "xpute/cpp/xpute/kit/math/scalar.test.cpp",
];
const NOT_EVEN: [&str; 4] = [
    r"\bMath\.round\(",
    r"\bstd::round\b",
    // A number's text rounds its last place half up: `fixed` (ts/client ui/shared/fixed.ts) rounds it half to even.
    r"\.toFixed\(",
    r"\.toPrecision\(",
];
/// A half added inside a floor's own parentheses, or before `.floor()`. Clippy can name no
/// such call, so Rust's are refused here too.
const HALF_FLOORED: [&str; 2] = [
    r"\bfloor\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\+\s*0\.5\s*\)",
    r"\([^()]*\+\s*0\.5\s*\)\.floor\(\)",
];

/// One order of dependency over a folder's modules: a module names only what is at or below
/// it, and no head names another's, a head being the modules of one side, its top and what
/// stands over it there; a module beside the order names anything, and only the order's top
/// names it. `root` is where the folder's files are, and `path` is what a
/// Rust file names its modules by, a `domain::` step past it, and the `layer::` before the
/// query that is its child in the module tree and above it in the order, being passed over (a
/// TypeScript file names a module by its file: `imported`). A name past `path` that no module
/// holds is the folder's root's own, which stands at `own`, or, where `own` is none, is not
/// read.
struct Order {
    root: &'static str,
    path: &'static str,
    levels: &'static [(&'static str, u8)],
    heads: &'static [&'static [&'static str]],
    beside: &'static [&'static str],
    own: Option<u8>,
}

/// The core's order (`ARCHITECTURE.md`, "The core is one order of dependency"): a module names
/// only what is at or below it, and neither head names the other. The top wires everything,
/// the UI's protocol among it. The settings, and an edit not yet made, which the top hands
/// from control to the scene, stand where both heads may name them. The profiler reads anything and stands beside the order, in none of its steps, and
/// only the top, which records into it, names it. The link's bindings and the values the
/// specs hold (`spec`), which name nothing of the core, stand under everything that reads
/// them.
const CORE: &str = "rust/client/core/src/";
const CORE_ORDER: [(&str, u8); 16] = [
    ("frame", 7),
    ("link", 7),
    ("ui", 7),
    ("pointer", 7),
    ("scene", 6),
    ("control", 6),
    ("smooth", 5),
    ("param", 5),
    ("preview", 5),
    ("setting", 5),
    ("view", 4),
    ("query", 3),
    ("layer", 2),
    ("sys", 1),
    ("abi", 0),
    ("spec", 0),
];
const CORE_HEADS: [&[&str]; 2] = [&["scene"], &["control"]];
const CORE_BESIDE: [&str; 1] = ["perf"];

/// Control's own order (domain/control.rs): the pointer's edits and its
/// gestures, then navigation, the camera, and under them where the pivot
/// stands.
const CONTROL_ORDER: [(&str, u8); 6] = [
    ("edit", 4),
    ("input", 4),
    ("navigation", 3),
    ("camera", 2),
    ("support", 1),
    ("param", 0),
];

/// The camera's own order (domain/control/camera.rs). At the bottom are the camera itself,
/// the folder's root, and beside it a gesture's anchor; above them, what the controls ask;
/// then the orbit and how far terrain lets it go, the first-person camera, with the tests'
/// fixture that places them; and
/// above them, which controller moves the camera.
const CAMERA_ORDER: [(&str, u8); 7] = [
    ("controller", 3),
    ("orbit", 2),
    ("collision", 2),
    ("first_person", 2),
    ("fixture", 2),
    ("state", 1),
    ("gesture", 0),
];

/// The view's own order (domain/view.rs): demand and the lens over the
/// observation, over the ground under a point and the frame.
const VIEW_ORDER: [(&str, u8); 6] = [
    ("demand", 3),
    ("lens", 3),
    ("observation", 2),
    ("above_ground", 1),
    ("frame", 1),
    ("param", 0),
];

/// The scene's own order (domain/scene.rs): the stage, the frame's top, over
/// the root, which sequences the parts that draw (`SCENE_OWN`), over the
/// markers, the characters and the indicators, over what they draw with,
/// each part keeping its own state.
const SCENE_ORDER: [(&str, u8); 16] = [
    ("stage", 7),
    ("marker", 5),
    ("character", 4),
    ("indicator", 4),
    ("environment", 3),
    ("gizmo", 3),
    ("decor", 2),
    ("model", 1),
    ("animation", 0),
    ("cast", 0),
    ("column", 0),
    ("frame", 0),
    ("param", 0),
    ("rate", 0),
    ("resolution", 0),
    ("transform", 0),
];
const SCENE_OWN: u8 = 6;

/// pxl-engine's order: what every part speaks, the layers' storage and
/// the world's fields, the query over them (ARCHITECTURE.md, "The query, the
/// only seam"), and what asks it. A layer knows no query, and the query no
/// asker.
const ENGINE: &str = "rust/client/engine/src/";
const ENGINE_ORDER: [(&str, u8); 21] = [
    ("param", 0),
    ("seed", 0),
    ("content", 0),
    ("answer", 0),
    ("count", 0),
    ("ground", 1),
    ("clipmap", 1),
    ("way", 1),
    ("fill", 1),
    ("object", 1),
    ("world", 2),
    ("cast", 2),
    ("support", 2),
    ("node_cache", 2),
    ("beam", 3),
    ("eye", 3),
    ("raster", 3),
    ("record", 3),
    ("ray", 3),
    ("light", 3),
    ("face", 3),
];

/// pxl-spec's order: what stands on nothing; the space; what a tile's cells hold and what the
/// catalog is; the value space laid over the catalog's palette, and the ways a column makes
/// over them; a brick over the ways, and a tile's place; and the bake's formats on top.
const SPEC: &str = "rust/spec/src/";
const SPEC_ORDER: [(&str, u8); 16] = [
    ("color", 0),
    ("share", 0),
    ("directory", 0),
    ("division", 0),
    ("route", 0),
    ("search", 0),
    ("space", 1),
    ("param", 2),
    ("ground", 2),
    ("asset", 2),
    ("object", 3),
    ("value", 3),
    ("way", 3),
    ("brick", 4),
    ("procedural", 4),
    ("bake", 5),
];

/// The space's own order (space.rs): positions, the hierarchy and the camera over them, and
/// how a layer's tiles map onto those.
const SPACE_ORDER: [(&str, u8); 4] = [("coord", 0), ("hierarchy", 1), ("camera", 1), ("layer", 2)];

/// A layer's own order (space/layer.rs): where its data ends and its nodes' names, the pyramid
/// of tiles, the lattice of cells placed on it, and the demand over them.
const LAYER_ORDER: [(&str, u8); 5] = [
    ("area", 0),
    ("octant", 0),
    ("quadrant", 1),
    ("lattice", 2),
    ("demand", 3),
];

/// pxl-content's order: its parameters and the one rule by which a count is handed down,
/// then its rules and fields, then the decor's generation over its scatter.
const CONTENT: &str = "rust/client/content/src/";
const CONTENT_ORDER: [(&str, u8); 7] = [
    ("param", 0),
    ("halve", 0),
    ("cave", 1),
    ("strata", 1),
    ("cloud", 1),
    ("scatter", 1),
    ("generation", 2),
];

/// pxl-render's order: what names no device, the look, the parameters and the values its
/// specs hold (the protocol's words, the GPU spans), under a frame's data, which the paints
/// stand on, under the device, the rays over it, what is drawn over them, and the renderer
/// on top.
const RENDER: &str = "rust/client/render/src/";
const RENDER_ORDER: [(&str, u8); 10] = [
    ("param", 0),
    ("look", 0),
    ("spec", 0),
    ("asset", 0),
    ("frame", 1),
    ("paint", 2),
    ("gpu", 3),
    ("ray", 4),
    ("pass", 5),
    ("renderer", 6),
];

/// A frame's own order (frame.rs): its shaders and the atlas's shelves, neither naming the
/// other.
const FRAME_ORDER: [(&str, u8); 2] = [("shader", 0), ("shelf", 0)];

/// The shaders' own order (frame/shader.rs): the words, the parts and WebGPU's floor, under the
/// pipeline records and their groups' layouts, under the frame's buffers, the groups it binds
/// over them and what a pass does with them, under the frame's calls.
const SHADER_ORDER: [(&str, u8); 8] = [
    ("debug_look", 0),
    ("floor", 0),
    ("part", 0),
    ("word", 0),
    ("record", 1),
    ("access", 2),
    ("group", 2),
    ("sequence", 3),
];

/// The device's own order (gpu.rs): what speaks no WebGPU, under the calls themselves, under
/// the draw lists, the pipelines and the surfaces, under the tiles, the atlas and what uploads,
/// under what is made with them: the decor's generation and the images.
const GPU_ORDER: [(&str, u8); 15] = [
    ("put", 0),
    ("split", 0),
    ("text", 0),
    ("api", 1),
    ("draw", 2),
    ("pipeline", 2),
    ("read", 2),
    ("surface", 2),
    ("atlas", 3),
    ("mip", 3),
    ("tile", 3),
    ("upload", 3),
    ("generator", 4),
    ("image_map", 4),
    ("pack", 4),
];
const GPU_OWN: u8 = 5;

/// What is drawn (pass.rs): the air and the screen's own passes, under what reads the air, under
/// what the model passes share, under the marks and the passes that stand alone, under those
/// that mark, under what draws through them.
const PASS_ORDER: [(&str, u8); 18] = [
    ("ao", 0),
    ("atmosphere", 0),
    ("finish", 0),
    ("propagate", 0),
    ("resolve", 0),
    ("cloud", 1),
    ("shadow", 1),
    ("sky", 1),
    ("star", 1),
    ("model", 2),
    ("debug", 3),
    ("mark", 3),
    ("marker", 3),
    ("decor", 4),
    ("location", 4),
    ("skinned_mesh", 4),
    ("character", 5),
    ("impostor", 5),
];
const PASS_OWN: u8 = 6;

/// The client's order (`ts/client/src`, `AGENTS.md`): its entry and the check of
/// its platform files over the UI and the wiring that opens the host, the two
/// heads; under them the services; and at the bottom the numbers both read
/// and the build's values.
const CLIENT: &str = "ts/client/src/";
const CLIENT_ORDER: [(&str, u8); 10] = [
    ("app", 3),
    ("conform", 3),
    ("boot", 3),
    ("page", 3),
    ("ui", 2),
    ("runtime", 2),
    ("service", 1),
    ("abi", 0),
    ("build", 0),
    ("remote-console", 0),
];
const CLIENT_HEADS: [&[&str]; 2] = [&["ui"], &["runtime"]];

/// The UI's own order (ts/client/src/ui): what assembles the features over them, and
/// under them what every feature shares.
const UI: &str = "ts/client/src/ui/";
const UI_ORDER: [(&str, u8); 4] = [("layout", 2), ("modal", 2), ("feature", 1), ("shared", 0)];

/// The services' own order (ts/client/src/service): the client's side of the
/// app's protocol, the one way to the core; the files the core opens; the
/// server they are read from; and what each platform gives.
const SERVICE: &str = "ts/client/src/service/";
const SERVICE_ORDER: [(&str, u8); 4] = [("core", 3), ("sys", 2), ("origin", 1), ("platform", 0)];

/// Where TypeScript's aliases are (`paths`), every reader's.
const TSCONFIG: &str = "ts/tsconfig.json";

/// Every folder's order held. A folder whose root holds only its modules gives the root's own items a
/// level above all of them, so no part reaches back to it.
const ORDERS: [Order; 18] = [
    Order {
        root: CORE,
        path: "crate",
        levels: &CORE_ORDER,
        heads: &CORE_HEADS,
        beside: &CORE_BESIDE,
        own: None,
    },
    Order {
        root: ENGINE,
        path: "crate",
        levels: &ENGINE_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: CONTENT,
        path: "crate",
        levels: &CONTENT_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: SPEC,
        path: "crate",
        levels: &SPEC_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: "rust/spec/src/space/",
        path: "crate::space",
        levels: &SPACE_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: "rust/spec/src/space/layer/",
        path: "crate::space::layer",
        levels: &LAYER_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: CLIENT,
        path: "",
        levels: &CLIENT_ORDER,
        heads: &CLIENT_HEADS,
        beside: &[],
        own: None,
    },
    Order {
        root: UI,
        path: "",
        levels: &UI_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: SERVICE,
        path: "",
        levels: &SERVICE_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: "rust/client/core/src/domain/control/",
        path: "crate::domain::control",
        levels: &CONTROL_ORDER,
        heads: &[],
        beside: &[],
        own: Some(5),
    },
    Order {
        root: "rust/client/core/src/domain/control/camera/",
        path: "crate::domain::control::camera",
        levels: &CAMERA_ORDER,
        heads: &[],
        beside: &[],
        own: Some(0),
    },
    Order {
        root: "rust/client/core/src/domain/view/",
        path: "crate::domain::view",
        levels: &VIEW_ORDER,
        heads: &[],
        beside: &[],
        own: Some(4),
    },
    Order {
        root: "rust/client/core/src/domain/scene/",
        path: "crate::domain::scene",
        levels: &SCENE_ORDER,
        heads: &[],
        beside: &[],
        own: Some(SCENE_OWN),
    },
    Order {
        root: RENDER,
        path: "crate",
        levels: &RENDER_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: "rust/client/render/src/frame/",
        path: "crate::frame",
        levels: &FRAME_ORDER,
        heads: &[],
        beside: &[],
        own: Some(1),
    },
    Order {
        root: "rust/client/render/src/frame/shader/",
        path: "crate::frame::shader",
        levels: &SHADER_ORDER,
        heads: &[],
        beside: &[],
        own: Some(4),
    },
    Order {
        root: "rust/client/render/src/gpu/",
        path: "crate::gpu",
        levels: &GPU_ORDER,
        heads: &[],
        beside: &[],
        own: Some(GPU_OWN),
    },
    Order {
        root: "rust/client/render/src/pass/",
        path: "crate::pass",
        levels: &PASS_ORDER,
        heads: &[],
        beside: &[],
        own: Some(PASS_OWN),
    },
];

/// The core's sys/ stands below everything that draws: it names no
/// renderer (`pxl_render`), and, beside its order, no profiler
/// (`CORE_BESIDE`).
const SYS: &str = "rust/client/core/src/sys/";
const SYS_REFUSED: &str = r"\bpxl_render\b";

/// What the world shows is decided in angles, the camera grid's cell; a
/// pixel enters only at the screen (ARCHITECTURE.md, "The camera grid").
/// The core reads the view's CSS pixels through one module, the screen
/// (domain/view/frame/screen.rs), and only these name it: the view's own
/// frame, the camera's gestures, the markers' layout and selection, the
/// stage's pixels a unit and the UI. A path is a file, or a folder by its
/// trailing slash.
const SCREEN_USERS: [&str; 7] = [
    "rust/client/core/src/domain/view/frame/",
    "rust/client/core/src/domain/control/camera/controller.rs",
    "rust/client/core/src/domain/control/input.rs",
    "rust/client/core/src/domain/scene/marker/arrangement.rs",
    "rust/client/core/src/domain/scene/marker/selection.rs",
    "rust/client/core/src/domain/scene/stage.rs",
    "rust/client/core/src/ui.rs",
];

/// The lines of `text`, a core source, comments left out, that reach the
/// screen: a path through it (`screen::`), a group naming it (`frame::{…
/// screen …}`, over as many lines as it runs), a glob or an alias of the
/// frame that would reach it unnamed, or a re-export of it, which would lend
/// it to a module the list does not name.
fn screen_reached(text: &str) -> Vec<usize> {
    let code: String = text
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let reach = Regex::new(
        r"\bscreen::|\bframe::\{[^}]*\bscreen\b|\bframe::\*|\bframe\s+as\b|\bpub\s+use\b[^;]*\bscreen\b",
    )
    .expect("the screen's pattern");
    let mut lines: Vec<usize> = reach
        .find_iter(&code)
        .map(|m| code[..m.start()].matches('\n').count() + 1)
        .collect();
    lines.dedup();
    lines
}

/// xpute's words are the guest, its hosts and the link between them (AGENTS.md):
/// pixelet's appear nowhere under it.
const XPUTE: &str = "xpute/";
const PIXELET_WORDS: &str = r"(?i)\b(pixelet|pxl[-_]\w+)\b";

/// The entries xpute's module on a device calls, the program's start among them, are the
/// runtime's: every function its C ABI declares (`RUNTIME_ABI`), and the Kotlin class
/// `xpute.Runtime`'s JNI names (`RUNTIME_JNI`), is refused under a device's hosts, xpute's and
/// the GPU's (`HOSTS`).
const HOSTS: [&str; 2] = ["xpute/native/host/", "xpute/gpu/native/host/"];
const RUNTIME_ABI: &str = "xpute/native/runtime/expo/ios/CXpute/xpute.h";
const RUNTIME_JNI: &str = r"\bJava_xpute_Runtime_\w+";

/// The functions `header` declares, a line's first name followed by its parenthesis.
fn declared(header: &str) -> Vec<String> {
    let decl = Regex::new(r"^\w[^(/]*?\b(xpute_\w+)\(").expect("a declaration's pattern");
    header
        .lines()
        .filter_map(|l| decl.captures(l))
        .map(|c| c[1].to_owned())
        .collect()
}

/// xpute's order, over its parts rather than a folder's modules: a part is a crate (`XPUTE_CARGO`)
/// or a package (`XPUTE_WORKSPACE`), or one of each, by its name past `xpute-` or `@xpute/`. The
/// kit names nothing, and the link stands on it. The guest and each platform's host stand over the
/// link; the guest and each platform are heads, none naming another. The TypeScript host
/// (`web-host`) varies by language, not platform, so it is on no head: a page's runtime and a
/// device's UI end both take it. The GPU is a plugin, which none of xpute's own parts (the kit, the
/// link, the guest and the hosts) names, and which may name them: its link stands over them, and
/// each of its other parts over that, in its counterpart's head, a host over xpute's host. Each
/// platform's runtime stands over its host and the GPU's: it may name its host, and no host,
/// xpute's or the GPU's, names a runtime; the runtime gives its host what the host needs of it. The
/// GPU's UI end on a device (its Expo module) stands with the runtime's, which shares the runtime's
/// name. Over every part, what develops xpute and generates its code is a head of its own, which no
/// part names and which names only the kit and the links. A part is held by what it ships: its
/// manifest's dependencies, a dev-dependency left out, and the names its sources write, its tests
/// left out, which may take the golden records' reader.
const XPUTE_ORDER: [(&str, u8); 18] = [
    ("kit", 0),
    ("link", 1),
    ("guest", 2),
    ("web-host", 2),
    ("native-host", 2),
    ("gpu-link", 3),
    ("gpu-guest", 4),
    ("gpu-web-host", 4),
    ("gpu-native-host", 4),
    ("web-runtime", 5),
    ("native-runtime", 5),
    ("gpu-native-runtime", 5),
    ("golden", 6),
    ("gen", 6),
    ("link-gen", 6),
    ("gen-targets", 6),
    ("gpu-gen-targets", 6),
    ("tool", 6),
];
const XPUTE_HEADS: [&[&str]; 4] = [
    &["guest", "gpu-guest"],
    &["web-runtime", "gpu-web-host"],
    &[
        "native-runtime",
        "native-host",
        "gpu-native-host",
        "gpu-native-runtime",
    ],
    &[
        "golden",
        "gen",
        "link-gen",
        "gen-targets",
        "gpu-gen-targets",
        "tool",
    ],
];
const XPUTE_LAYERS: Order = Order {
    root: XPUTE,
    path: "",
    levels: &XPUTE_ORDER,
    heads: &XPUTE_HEADS,
    beside: &[],
    own: None,
};

/// The modules `text` names past `path`, a `domain::` step past it passed
/// over (the core's heads and trunk live under it) and the `layer::` before
/// `query` (a child of `layer` that stands above it), a group's every first
/// name among them, each with the line it is named on; `path` itself, named
/// whole, is its root, an empty name.
fn named(text: &str, path: &str) -> Vec<(usize, String)> {
    let code: String = text
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let ident = |s: &str| -> String { s.chars().take_while(|&c| word(c)).collect() };
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = code[at..].find(path) {
        let start = at + k;
        at = start + path.len();
        let rest = &code[at..];
        // `path` inside a longer name, as `crate::domain::fx` holds `crate::domain::f`, is no
        // reference to it.
        if rest.starts_with(word) || code[..start].ends_with(word) {
            continue;
        }
        let line = code[..start].matches('\n').count() + 1;
        let Some(rest) = rest.strip_prefix("::") else {
            out.push((line, String::new()));
            continue;
        };
        let rest = rest.strip_prefix("domain::").unwrap_or(rest);
        let rest = rest
            .strip_prefix("layer::")
            .filter(|r| ident(r) == "query")
            .unwrap_or(rest);
        if let Some(group) = rest.strip_prefix('{') {
            // A group: the first name of each item at its own depth.
            let (mut depth, mut item_start) = (0, true);
            for (i, c) in group.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' if depth == 0 => break,
                    '}' => depth -= 1,
                    ',' if depth == 0 => item_start = true,
                    c if item_start && word(c) => {
                        out.push((line, ident(&group[i..])));
                        item_start = false;
                    }
                    _ => {}
                }
            }
        } else {
            out.push((line, ident(rest)));
        }
    }
    out
}

/// Each alias `TSCONFIG` gives, as the name an import writes and the folder,
/// from the tree's root, it stands for.
fn ts_aliases(read: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let text: String = read(TSCONFIG)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let cfg: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let dir = TSCONFIG.rsplit_once('/').map_or("", |(d, _)| d);
    cfg["compilerOptions"]["paths"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, to)| {
            let to = to.get(0)?.as_str()?;
            Some((
                name.trim_end_matches('*').to_string(),
                format!(
                    "{dir}/{}",
                    to.trim_start_matches("./").trim_end_matches('*')
                ),
            ))
        })
        .collect()
}

/// The files `text`, the TypeScript file `f`, imports, exports from or
/// starts a worker on, comments left out, each a path from the tree's root
/// with the line it is named on: an alias (`aliases`, a prefix ending in `/`,
/// which a name met with its own `/` after it also takes, as a package's
/// bare name) put back as its folder, a relative name resolved from `f`'s; a
/// package no alias gives is left out.
fn imported(text: &str, f: &str, aliases: &[(String, String)]) -> Vec<(usize, String)> {
    let blank = |m: &regex::Captures| m[0].replace(|c| c != '\n', " ");
    let code = Regex::new(r"/\*[\s\S]*?\*/")
        .expect("a block comment's pattern")
        .replace_all(text, blank);
    let code = Regex::new(r"(?m)^\s*//.*$")
        .expect("a line comment's pattern")
        .replace_all(&code, blank);
    let name = Regex::new(r#"(?:\bfrom|\bimport\s*\(?|\bnew\s+URL\s*\()\s*["']([^"']+)["']"#)
        .expect("an import's pattern");
    let dir = f.rsplit_once('/').map_or("", |(d, _)| d);
    name.captures_iter(&code)
        .filter_map(|c| {
            let line = code[..c.get(0)?.start()].matches('\n').count() + 1;
            let path = if c[1].starts_with('.') {
                format!("{dir}/{}", &c[1])
            } else {
                let (alias, to) = aliases
                    .iter()
                    .find(|(a, _)| format!("{}/", &c[1]).starts_with(a.as_str()))?;
                format!("{to}{}", c[1].get(alias.len()..).unwrap_or(""))
            };
            let mut parts: Vec<&str> = Vec::new();
            for step in path.split('/') {
                match step {
                    "" | "." => {}
                    ".." => {
                        parts.pop();
                    }
                    step => parts.push(step),
                }
            }
            Some((line, parts.join("/")))
        })
        .collect()
}

/// Where `f` stands in `order` under `root`: its first step past `root`, or
/// past `domain/` for the core's heads and trunk.
fn standing(f: &str, root: &str) -> String {
    let rest = &f[root.len()..];
    let rest = rest.strip_prefix("domain/").unwrap_or(rest);
    rest.split('/')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .to_string()
}

/// What a file names, as `out_of_order` reads it: the file, the module or part it stands as,
/// and each module or part it names with the line it names it on.
type Uses = (String, String, Vec<(usize, String)>);

/// What each of `o`'s files names (`Uses`), where each stands as the module it is.
fn folder_uses(files: &[&str], read: &dyn Fn(&str) -> Option<String>, o: &Order) -> Vec<Uses> {
    let aliases = ts_aliases(read);
    files
        .iter()
        .filter(|f| f.starts_with(o.root) && [".rs", ".ts", ".tsx"].iter().any(|e| f.ends_with(e)))
        .map(|f| {
            let me = standing(f, o.root);
            let text = read(f).unwrap_or_default();
            let names = match f.ends_with(".rs") {
                true => named(&text, o.path),
                false => imported(&text, f, &aliases)
                    .into_iter()
                    .filter(|(_, p)| p.starts_with(o.root))
                    .map(|(line, p)| (line, standing(&p, o.root)))
                    .collect(),
            };
            (f.to_string(), me, names)
        })
        .collect()
}

/// Every name in `uses` (`Uses`) of a module above the file's own, of one head's module by
/// another head's, or of one beside the order off its top; and every module the order does not
/// place, at the first file of it, but the crate's root and those that stand beside it, so a
/// new one is placed before it is checked.
fn out_of_order(o: &Order, uses: Vec<Uses>) -> Vec<String> {
    let level = |m: &str| o.levels.iter().find(|(n, _)| *n == m).map(|(_, l)| *l);
    let head = |m: &str| o.heads.iter().position(|h| h.contains(&m));
    let top = o.levels.iter().map(|&(_, l)| l).max().unwrap_or(0);
    let mut problems = Vec::new();
    let mut unplaced = std::collections::BTreeMap::new();
    for (f, me, names) in uses {
        let Some(mine) = level(&me) else {
            if !["lib", "domain"].contains(&me.as_str()) && !o.beside.contains(&me.as_str()) {
                unplaced.entry(me).or_insert(f);
            }
            continue;
        };
        for (line, them) in names {
            if them == me {
                continue;
            }
            if o.beside.contains(&them.as_str()) {
                if mine < top {
                    problems.push(format!(
                        "{f}:{line}: `{me}` names `{them}`, which stands beside the order and only its top names"
                    ));
                }
                continue;
            }
            let Some(theirs) = level(&them).or(o.own) else {
                continue;
            };
            let apart = head(&me).zip(head(&them)).is_some_and(|(a, b)| a != b);
            if theirs > mine || apart {
                let them = if level(&them).is_some() {
                    format!("`{them}`")
                } else {
                    format!("its folder's root (`{}`)", o.path)
                };
                problems.push(format!(
                    "{f}:{line}: `{me}` names {them}, above it or on another head, in the layers' order"
                ));
            }
        }
    }
    for (me, f) in unplaced {
        problems.push(format!(
            "{f}: `{me}` has no place in the layers' order; give it one (rule.rs)"
        ));
    }
    problems
}

/// The lines of `text`, a sys/ source, comments left out, that name what
/// sys/ stands below (`SYS_REFUSED`).
fn sys_refused(text: &str) -> Vec<usize> {
    let refused = Regex::new(SYS_REFUSED).expect("sys's refused pattern");
    text.lines()
        .enumerate()
        .filter(|(_, l)| refused.is_match(l.split("//").next().unwrap_or("")))
        .map(|(n, _)| n + 1)
        .collect()
}

/// Each tracked `.rs` file that no crate reaches: it is none of its own crate's roots
/// (`lib.rs`, `main.rs`, `build.rs`, a file under `src/bin/`, `tests/`, `benches/` or
/// `examples/`), and no `mod`, `#[path]` or `include!` in a file, nor a manifest's `path`,
/// names it. Such a file compiles in nothing, so nothing checks it.
fn unlinked(files: &[&str], read: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    use std::collections::BTreeSet;
    use std::path::{Component, Path, PathBuf};
    fn clean(p: &Path) -> PathBuf {
        let mut out = PathBuf::new();
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                c => out.push(c),
            }
        }
        out
    }
    let rust =
        |f: &&&str| f.ends_with(".rs") && (f.starts_with("rust/") || f.starts_with("xpute/"));
    let module = Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;")
        .expect("a module's pattern");
    let path_attr = Regex::new(r#"#\[path\s*=\s*"([^"]+)"\]"#).expect("a path's pattern");
    let include = Regex::new(r#"include!\(\s*"([^"]+\.rs)"\s*\)"#).expect("an include's pattern");
    let manifest_path =
        Regex::new(r#"(?m)^\s*path\s*=\s*"([^"]+\.rs)""#).expect("a manifest path's pattern");
    let mut reached: BTreeSet<PathBuf> = BTreeSet::new();
    for f in files.iter().filter(|f| f.ends_with("Cargo.toml")) {
        let dir = Path::new(f).parent().unwrap_or(Path::new(""));
        for c in manifest_path.captures_iter(&read(f).unwrap_or_default()) {
            reached.insert(clean(&dir.join(&c[1])));
        }
    }
    for f in files.iter().filter(rust) {
        let file = Path::new(f);
        let dir = file.parent().unwrap_or(Path::new(""));
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // A `mod` in a crate root or a `mod.rs` names files beside it; in any
        // other file, files in the folder named as that file is.
        let children = if ["lib", "main", "mod", "build"].contains(&stem) {
            dir.to_path_buf()
        } else {
            dir.join(stem)
        };
        let text = read(f).unwrap_or_default();
        for c in path_attr.captures_iter(&text) {
            reached.insert(clean(&dir.join(&c[1])));
        }
        for c in include.captures_iter(&text) {
            reached.insert(clean(&dir.join(&c[1])));
        }
        for c in module.captures_iter(&text) {
            reached.insert(children.join(format!("{}.rs", &c[1])));
            reached.insert(children.join(&c[1]).join("mod.rs"));
        }
    }
    files
        .iter()
        .filter(rust)
        .filter(|f| {
            let p = Path::new(f);
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let rooted = ["lib.rs", "main.rs", "build.rs"].contains(&name)
                || ["/src/bin/", "/tests/", "/benches/", "/examples/"]
                    .iter()
                    .any(|d| f.contains(d));
            !rooted && !reached.contains(p)
        })
        .map(|f| format!("{f}: no crate reaches it: no `mod`, `#[path]`, `include!` or manifest `path` names it, so it compiles in nothing"))
        .collect()
}

/// The dependency cycles let stand, each as its graph and its members in order. pxl-tool's
/// waits for the catalog's redesign, which changes its edges. No other cycle stands, and one
/// of these that gains or loses a member is another.
const CYCLES_KNOWN: [&str; 1] = ["rust/tool: spec::asset spec::catalog spec::ingest"];

/// The workspace file that lists xpute's TypeScript packages, the one source every reader of
/// them takes, and pixelet's, which must list the same for pnpm.
const XPUTE_WORKSPACE: &str = "xpute/pnpm-workspace.yaml";
const TS_WORKSPACE: &str = "ts/pnpm-workspace.yaml";

/// The directories a pnpm-workspace.yaml lists under `packages:`, one `  - <dir>` a line up to
/// the next blank line, or why it cannot be read so: an entry with a glob, a quote or a
/// comment refuses it.
fn workspace_dirs(
    read: &dyn Fn(&str) -> Option<String>,
    file: &str,
) -> Result<Vec<String>, String> {
    let text = read(file).ok_or(format!("{file}: unreadable"))?;
    let entry = Regex::new(r"^  - ([A-Za-z0-9_./-]+)$").expect("a workspace entry's pattern");
    let after = text
        .split_once("\npackages:\n")
        .ok_or(format!("{file}: no `packages:`"))?
        .1;
    let dirs: Vec<String> = after
        .lines()
        .take_while(|l| !l.is_empty())
        .map(|l| {
            entry
                .captures(l)
                .map(|c| c[1].to_string())
                .ok_or(format!("{file}: `{l}` is not `  - <dir>`"))
        })
        .collect::<Result<_, _>>()?;
    if dirs.is_empty() {
        return Err(format!("{file}: lists no package"));
    }
    Ok(dirs)
}

/// xpute's TypeScript packages, each a directory its pnpm-workspace.yaml lists, by the name
/// its package.json gives, which an import writes, and the folder it stands for; or why one
/// cannot be read so.
fn xpute_packages(read: &dyn Fn(&str) -> Option<String>) -> Result<Vec<(String, String)>, String> {
    workspace_dirs(read, XPUTE_WORKSPACE)?
        .into_iter()
        .map(|d| {
            let manifest = format!("xpute/{d}/package.json");
            let text = read(&manifest).ok_or(format!("{manifest}: unreadable"))?;
            let json: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("{manifest}: {e}"))?;
            let name = json["name"]
                .as_str()
                .ok_or(format!("{manifest}: no name"))?;
            let src = format!("xpute/{d}/src/");
            if !root().join(&src).is_dir() {
                return Err(format!("xpute/{d}: no src/"));
            }
            Ok((name.to_string(), src))
        })
        .collect()
}

/// xpute's packages as `imported` takes them: each name an import writes, with its `/`, and
/// its sources' folder.
fn package_aliases(packages: &[(String, String)]) -> Vec<(String, String)> {
    packages
        .iter()
        .map(|(n, src)| (format!("{n}/"), src.clone()))
        .collect()
}

/// The Cargo workspace that lists xpute's crates.
const XPUTE_CARGO: &str = "xpute/Cargo.toml";

/// pixelet's workspace, which builds xpute's crates by path. It patches crates.io in its own
/// root, as `[patch]` holds only there, with xpute's table (`patches_agree`, xpute/wgpu-fork.md).
const RUST_CARGO: &str = "rust/Cargo.toml";

/// The entries of `manifest`'s `[patch.crates-io]` table, its comment lines and blank lines
/// left out; none where it has no such table.
fn patch_table(manifest: &str) -> Option<Vec<&str>> {
    let mut lines = manifest
        .lines()
        .skip_while(|l| l.trim() != "[patch.crates-io]");
    lines.next()?;
    Some(
        lines
            .take_while(|l| !l.trim_start().starts_with('['))
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect(),
    )
}

/// xpute's crates, each a directory `XPUTE_CARGO`'s `members` lists, by the name its manifest
/// gives and its sources' folder; or why one cannot be read so.
fn xpute_crates(read: &dyn Fn(&str) -> Option<String>) -> Result<Vec<(String, String)>, String> {
    let text = read(XPUTE_CARGO).ok_or(format!("{XPUTE_CARGO}: unreadable"))?;
    let members = text
        .split_once("\nmembers = [")
        .and_then(|(_, m)| m.split_once(']'))
        .ok_or(format!("{XPUTE_CARGO}: no `members = [...]`"))?
        .0;
    let member = Regex::new(r#""([^"]+)""#).expect("a member's pattern");
    let name = Regex::new(r#"(?m)^name\s*=\s*"([^"]+)""#).expect("a crate name's pattern");
    member
        .captures_iter(members)
        .map(|c| {
            let manifest = format!("xpute/{}/Cargo.toml", &c[1]);
            let text = read(&manifest).ok_or(format!("{manifest}: unreadable"))?;
            let n = name.captures(&text).ok_or(format!("{manifest}: no name"))?;
            let src = format!("xpute/{}/src/", &c[1]);
            if !root().join(&src).is_dir() {
                return Err(format!("xpute/{}: no src/", &c[1]));
            }
            Ok((n[1].to_string(), src))
        })
        .collect()
}

/// What each of xpute's manifests and sources names of its parts, as `out_of_order` reads it: the
/// file, the part it is of, and each part it names with the line: a manifest's dependencies
/// but its dev-dependencies; in a Rust source, a crate's name; in a TypeScript source, an
/// import of a package or of another part's file. A test is left out. A part's manifest
/// comes first, so a part the order does not place is reported at it.
fn xpute_uses(
    files: &[&str],
    read: &dyn Fn(&str) -> Option<String>,
    crates: &[(String, String)],
    packages: &[(String, String)],
) -> Vec<Uses> {
    let part = |name: &str| -> String {
        let name = name.replace('_', "-");
        name.trim_start_matches("xpute-")
            .trim_start_matches("@xpute/")
            .to_string()
    };
    let part_of = |path: &str| -> Option<String> {
        crates
            .iter()
            .chain(packages)
            .find(|(_, src)| format!("{path}/").starts_with(src.as_str()))
            .map(|(n, _)| part(n))
    };
    let aliases = package_aliases(packages);
    let table =
        Regex::new(r"^\[(?:target\..+\.)?(?:build-)?dependencies(?:\.(xpute-[a-z0-9-]+))?\]$")
            .expect("a dependency table's pattern");
    let dep = Regex::new(r#"^(xpute-[a-z0-9-]+)\s*[.=]|\bpackage\s*=\s*"(xpute-[a-z0-9-]+)""#)
        .expect("a dependency's pattern");
    let krate = Regex::new(r"\bxpute_[a-z0-9_]+\b").expect("a crate's pattern");
    let mut uses = Vec::new();
    for (name, src) in crates {
        let manifest = format!("{}Cargo.toml", src.trim_end_matches("src/"));
        let mut names = Vec::new();
        let mut on = false;
        for (n, l) in read(&manifest).unwrap_or_default().lines().enumerate() {
            let l = l.split('#').next().unwrap_or("").trim();
            if l.starts_with('[') {
                let c = table.captures(l);
                on = c.is_some();
                names.extend(c.and_then(|c| c.get(1)).map(|d| (n + 1, part(d.as_str()))));
            } else if let Some(c) = dep.captures(l).filter(|_| on) {
                names.extend(c.get(1).or(c.get(2)).map(|d| (n + 1, part(d.as_str()))));
            }
        }
        uses.push((manifest, part(name), names));
    }
    for (name, src) in packages {
        let manifest = format!("{}package.json", src.trim_end_matches("src/"));
        let text = read(&manifest).unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let mut names = Vec::new();
        for key in ["dependencies", "peerDependencies", "optionalDependencies"] {
            let at = lines
                .iter()
                .position(|l| l.contains(&format!("\"{key}\"")))
                .unwrap_or(0);
            for d in json[key]
                .as_object()
                .into_iter()
                .flat_map(|o| o.keys())
                .filter(|d| d.starts_with("@xpute/"))
            {
                let line = lines[at..]
                    .iter()
                    .position(|l| l.contains(&format!("\"{d}\"")))
                    .map_or(0, |k| at + k + 1);
                names.push((line, part(d)));
            }
        }
        uses.push((manifest, part(name), names));
    }
    for f in files.iter().filter(|f| !f.contains(".test.")) {
        let Some(me) = part_of(f) else { continue };
        let text = read(f).unwrap_or_default();
        let names: Vec<(usize, String)> = if f.ends_with(".rs") {
            let code: String = text
                .lines()
                .map(|l| l.split("//").next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n");
            krate
                .find_iter(&code)
                .filter(|m| crates.iter().any(|(n, _)| part(n) == part(m.as_str())))
                .map(|m| {
                    (
                        code[..m.start()].matches('\n').count() + 1,
                        part(m.as_str()),
                    )
                })
                .collect()
        } else if f.ends_with(".ts") || f.ends_with(".tsx") {
            imported(&text, f, &aliases)
                .into_iter()
                .filter_map(|(line, p)| Some((line, part_of(&p)?)))
                .collect()
        } else {
            continue;
        };
        uses.push((f.to_string(), me, names));
    }
    uses
}

/// Whether pixelet's workspace lists xpute's packages as xpute's own does, in its order.
fn workspaces_agree(read: &dyn Fn(&str) -> Option<String>) -> Result<(), String> {
    let xpute = workspace_dirs(read, XPUTE_WORKSPACE)?;
    let listed: Vec<String> = workspace_dirs(read, TS_WORKSPACE)?
        .into_iter()
        .filter_map(|d| d.strip_prefix("../xpute/").map(String::from))
        .collect();
    if listed != xpute {
        return Err(format!(
            "{TS_WORKSPACE}: its ../xpute/ entries are not {XPUTE_WORKSPACE}'s list"
        ));
    }
    Ok(())
}

/// Whether pixelet's workspace patches crates.io as xpute's does: one table in both, or none
/// in either.
fn patches_agree(read: &dyn Fn(&str) -> Option<String>) -> Result<(), String> {
    let (rust, xpute) = (read(RUST_CARGO), read(XPUTE_CARGO));
    if rust.as_deref().and_then(patch_table) != xpute.as_deref().and_then(patch_table) {
        return Err(format!(
            "{RUST_CARGO}: its [patch.crates-io] table is not {XPUTE_CARGO}'s"
        ));
    }
    Ok(())
}

/// The strongly connected parts of `g` with more than one member, each sorted.
fn strong_parts(
    g: &std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
) -> Vec<Vec<String>> {
    struct Walk<'a> {
        g: &'a std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
        idx: std::collections::BTreeMap<&'a str, usize>,
        low: std::collections::BTreeMap<&'a str, usize>,
        stack: Vec<&'a str>,
        on: std::collections::BTreeSet<&'a str>,
        parts: Vec<Vec<String>>,
    }
    fn visit<'a>(w: &mut Walk<'a>, v: &'a str) {
        let n = w.idx.len();
        w.idx.insert(v, n);
        w.low.insert(v, n);
        w.stack.push(v);
        w.on.insert(v);
        for u in w.g.get(v).into_iter().flatten() {
            let u = u.as_str();
            if !w.g.contains_key(u) {
                continue;
            }
            if !w.idx.contains_key(u) {
                visit(w, u);
                let l = w.low[v].min(w.low[u]);
                w.low.insert(v, l);
            } else if w.on.contains(u) {
                let l = w.low[v].min(w.idx[u]);
                w.low.insert(v, l);
            }
        }
        if w.low[v] == w.idx[v] {
            let mut part = Vec::new();
            while let Some(u) = w.stack.pop() {
                w.on.remove(u);
                part.push(u.to_string());
                if u == v {
                    break;
                }
            }
            if part.len() > 1 {
                part.sort();
                w.parts.push(part);
            }
        }
    }
    let mut w = Walk {
        g,
        idx: Default::default(),
        low: Default::default(),
        stack: Vec::new(),
        on: Default::default(),
        parts: Vec::new(),
    };
    for v in g.keys() {
        if !w.idx.contains_key(v.as_str()) {
            visit(&mut w, v);
        }
    }
    w.parts
}

/// Each crate's module graph, by its `crate::`, `super::` and `self::` paths, and the
/// TypeScript files' graph, by their imports, as a page resolves them and as iOS and Android
/// do (`.ios` or `.android`, then `.native`, before the page's own file). A test's file, a
/// generated spec and the crate's root, which declares every module, are no node of it. Each
/// cycle not in `CYCLES_KNOWN`.
fn cycles(
    files: &[&str],
    read: &dyn Fn(&str) -> Option<String>,
    packages: &[(String, String)],
) -> Vec<String> {
    use std::collections::{BTreeMap, BTreeSet};
    let mut found: BTreeSet<String> = BTreeSet::new();
    let path = Regex::new(r"\b(crate|(?:super::)*super|self)::(?:\{([^}]*)\}|((?:[a-z_0-9]+::)*[a-z_0-9]+)(?:::\{([^}]*)\})?)").expect("a module path's pattern");
    let comment = Regex::new(r"(?m)//.*$").expect("a comment's pattern");
    let roots = files.iter().filter(|f| {
        f.ends_with("/src/lib.rs") && (f.starts_with("rust/") || f.starts_with("xpute/"))
    });
    for lib in roots {
        let src = lib.trim_end_matches("lib.rs");
        let name = src.trim_end_matches("/src/");
        let module = |f: &str| -> String {
            let m = f[src.len()..]
                .trim_end_matches(".rs")
                .trim_end_matches("/mod");
            if m == "lib" {
                String::new()
            } else {
                m.replace('/', "::")
            }
        };
        let mods: Vec<&str> = files
            .iter()
            .copied()
            .filter(|f| {
                f.starts_with(src)
                    && f.ends_with(".rs")
                    && !f[src.len()..].starts_with("bin/")
                    && !f.ends_with(".test.rs")
                    && !f.ends_with(".spec.rs")
            })
            .collect();
        let names: BTreeSet<String> = mods.iter().map(|f| module(f)).collect();
        let owner = |p: &str| -> String {
            let mut p = p.to_string();
            while !names.contains(&p) {
                match p.rsplit_once("::") {
                    Some((head, _)) => p = head.to_string(),
                    None => return String::new(),
                }
            }
            p
        };
        let mut g: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for f in &mods {
            let m = module(f);
            let text = read(f).unwrap_or_default();
            let code = comment.replace_all(&text, "");
            let mut deps = BTreeSet::new();
            for c in path.captures_iter(&code) {
                // Where the path starts: the root, or the file's module and as
                // many steps up as it says `super`.
                let base: Vec<&str> = match &c[1] {
                    "crate" => Vec::new(),
                    "self" => m.split("::").filter(|s| !s.is_empty()).collect(),
                    supers => {
                        let up = supers.matches("super").count();
                        let steps: Vec<&str> = m.split("::").filter(|s| !s.is_empty()).collect();
                        steps[..steps.len().saturating_sub(up)].to_vec()
                    }
                };
                let join = |tail: &str| -> String {
                    let mut p = base.join("::");
                    for t in tail.split("::").filter(|t| !t.is_empty()) {
                        if !p.is_empty() {
                            p.push_str("::");
                        }
                        p.push_str(t);
                    }
                    p
                };
                let head = join(c.get(3).map_or("", |p| p.as_str()));
                deps.insert(owner(&head));
                let items = c.get(2).or(c.get(4)).map_or("", |i| i.as_str());
                for item in items.split(',') {
                    let item = item
                        .trim()
                        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                        .next()
                        .unwrap_or("");
                    if !item.is_empty() && item != "self" {
                        let p = if head.is_empty() {
                            item.to_string()
                        } else {
                            format!("{head}::{item}")
                        };
                        deps.insert(owner(&p));
                    }
                }
            }
            deps.retain(|d| !d.is_empty() && *d != m);
            g.insert(m, deps);
        }
        for part in strong_parts(&g) {
            found.insert(format!("{name}: {}", part.join(" ")));
        }
    }
    let mut aliases = ts_aliases(read);
    aliases.extend(package_aliases(packages));
    let ts: Vec<&str> = files
        .iter()
        .copied()
        .filter(|f| {
            (f.starts_with("ts/client/src/") || f.starts_with("xpute/"))
                && (f.ends_with(".ts") || f.ends_with(".tsx"))
                && !f.contains(".test.")
                && f.contains("/src/")
        })
        .collect();
    let have: BTreeSet<&str> = ts.iter().copied().collect();
    // The page's own files, then each device's, its overrides found first.
    let platforms: [(&[&str], &[&str]); 3] = [
        (&[], &[".native.", ".ios.", ".android."]),
        (&[".ios", ".native"], &[".android."]),
        (&[".android", ".native"], &[".ios."]),
    ];
    for (over, apart) in platforms {
        let resolve = |p: &str| -> Option<String> {
            let stem = p.trim_end_matches(".tsx").trim_end_matches(".ts");
            over.iter()
                .flat_map(|o| [format!("{stem}{o}.tsx"), format!("{stem}{o}.ts")])
                .chain([
                    p.to_string(),
                    format!("{p}.ts"),
                    format!("{p}.tsx"),
                    format!("{p}/mod.ts"),
                ])
                .find(|t| have.contains(t.as_str()))
        };
        let mut g: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for f in ts.iter().filter(|f| !apart.iter().any(|a| f.contains(a))) {
            let text = read(f).unwrap_or_default();
            let mut deps: BTreeSet<String> = imported(&text, f, &aliases)
                .into_iter()
                .filter_map(|(_, p)| resolve(&p))
                .collect();
            deps.remove(*f);
            g.insert(f.to_string(), deps);
        }
        for part in strong_parts(&g) {
            found.insert(format!("ts: {}", part.join(" ")));
        }
    }
    found
        .into_iter()
        .filter(|c| !CYCLES_KNOWN.contains(&c.as_str()))
        .map(|c| format!("{c}: a dependency cycle; one of its members owns what the others need, and they reach it one way (rule.rs `cycles`)"))
        .collect()
}

/// The tree's files, one path a line from `root`: git's files, tracked and not ignored, where
/// `root` is a checkout's own top; otherwise, as in an export laid out under a checkout's
/// ignored target, every file under it except the builds' and the packages', of which git
/// there would list none. Outside any checkout, an error.
fn list_files(root: &std::path::Path) -> Result<String, String> {
    let top = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git rev-parse: {e}"))?;
    if !top.status.success() {
        return Err("git rev-parse: not in a checkout".into());
    }
    let top = String::from_utf8(top.stdout).map_err(|e| format!("git rev-parse: {e}"))?;
    let own = std::path::Path::new(top.trim())
        .canonicalize()
        .is_ok_and(|t| root.canonicalize().is_ok_and(|r| r == t));
    if own {
        let out = Command::new("git")
            .args(["ls-files", "-co", "--exclude-standard"])
            .current_dir(root)
            .output()
            .map_err(|e| format!("git ls-files: {e}"))?;
        return String::from_utf8(out.stdout).map_err(|e| format!("git ls-files: {e}"));
    }
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut String) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name();
            if ["target", "node_modules", ".git"]
                .iter()
                .any(|n| name == *n)
            {
                continue;
            }
            // A link is not followed, as git lists a link and not what it names.
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                walk(&p, root, out);
            } else if let Ok(rel) = p.strip_prefix(root) {
                out.push_str(&rel.to_string_lossy());
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    walk(root, root, &mut out);
    Ok(out)
}

fn run() -> Result<Vec<String>, String> {
    let root = root();
    let listed = list_files(&root)?;
    // The tree as it stands: a file deleted and not yet committed is gone, so
    // a removal is checked before it is committed.
    let files: Vec<&str> = listed
        .lines()
        .filter(|f| !f.is_empty() && !f.contains("node_modules/") && root.join(f).is_file())
        .collect();
    let read = |f: &str| std::fs::read_to_string(root.join(f)).ok();
    let mut problems = Vec::new();

    let cast = Regex::new(r"\bas\s+\*(const|mut)\b").expect("the cast's pattern");
    let comment = Regex::new(r"^\s*//").expect("the comment's pattern");
    for f in files
        .iter()
        .filter(|f| f.ends_with(".rs") && ADDR_CRATES.iter().any(|c| f.starts_with(c)))
    {
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            if !comment.is_match(line) && cast.is_match(line) {
                problems.push(format!(
                    "{f}:{}: casts to a pointer where an address goes through xpute_link::mem: {}",
                    n + 1,
                    line.trim()
                ));
            }
        }
    }

    let patterns = |ps: &[&str]| -> Vec<Regex> {
        ps.iter()
            .map(|p| Regex::new(p).expect("a rounding's pattern"))
            .collect()
    };
    let (not_even, half_floored) = (patterns(&NOT_EVEN), patterns(&HALF_FLOORED));
    let rounded = Regex::new(r"\.(rs|tsx?|wgsl|hpp|cpp)$").expect("the extensions' pattern");
    for f in files.iter().filter(|f| {
        ROUNDING_ROOTS.iter().any(|r| f.starts_with(r))
            && !ROUNDING_VARIANT_FILES.contains(f)
            && rounded.is_match(f)
    }) {
        let calls = !f.ends_with(".rs");
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            if (calls && not_even.iter().any(|r| r.is_match(line)))
                || half_floored.iter().any(|r| r.is_match(line))
            {
                problems.push(format!(
                    "{f}:{}: rounds other than half to even: {}",
                    n + 1,
                    line.trim()
                ));
            }
        }
    }

    // The tree's scripts are ES modules, TypeScript where a loader takes it,
    // the bundlers' configs among them: Node runs only the native toolchain's
    // own scripts (CocoaPods, Xcode's and Gradle's phases).
    for f in files
        .iter()
        .filter(|f| f.ends_with(".cjs") || f.ends_with(".cts"))
    {
        problems.push(format!("{f}: CommonJS; the tree's scripts are ES modules"));
    }

    for o in &ORDERS {
        problems.extend(out_of_order(o, folder_uses(&files, &read, o)));
    }
    if ts_aliases(&read).is_empty() {
        problems.push(format!(
            "{TSCONFIG}: no alias read (`compilerOptions.paths`), so no import through one is held to the client's order"
        ));
    }
    for f in files
        .iter()
        .filter(|f| f.starts_with(SYS) && f.ends_with(".rs"))
    {
        for line in sys_refused(&read(f).unwrap_or_default()) {
            problems.push(format!(
                "{f}:{line}: names the renderer under sys/, which stands below everything that draws (rule.rs `SYS_REFUSED`)"
            ));
        }
    }
    for f in files.iter().filter(|f| {
        f.starts_with(CORE) && f.ends_with(".rs") && !SCREEN_USERS.iter().any(|u| f.starts_with(u))
    }) {
        for line in screen_reached(&read(f).unwrap_or_default()) {
            problems.push(format!(
                "{f}:{line}: reaches the screen's pixels where the world is decided in angles; only SCREEN_USERS may (rule.rs)"
            ));
        }
    }
    let pixelet = Regex::new(PIXELET_WORDS).expect("pixelet's words' pattern");
    for f in files.iter().filter(|f| f.starts_with(XPUTE)) {
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            if pixelet.is_match(line) {
                problems.push(format!(
                    "{f}:{}: names pixelet's word under xpute: {}",
                    n + 1,
                    line.trim()
                ));
            }
        }
    }

    match read(RUNTIME_ABI)
        .map(|h| declared(&h))
        .filter(|d| !d.is_empty())
    {
        Some(entries) => {
            let named = Regex::new(&format!(r"\b({})\b|{RUNTIME_JNI}", entries.join("|")))
                .expect("the entries' pattern");
            for f in files
                .iter()
                .filter(|f| HOSTS.iter().any(|h| f.starts_with(h)))
            {
                for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
                    if let Some(m) = named.find(line) {
                        problems.push(format!(
                            "{f}:{}: a host names its runtime's entry `{}` (rule.rs `HOSTS`)",
                            n + 1,
                            m.as_str()
                        ));
                    }
                }
            }
        }
        None => problems.push(format!(
            "{RUNTIME_ABI}: no entry read, so no host is held to its runtime's"
        )),
    }

    problems.extend(unlinked(&files, &read));
    if let Err(e) = workspaces_agree(&read) {
        problems.push(e);
    }
    if let Err(e) = patches_agree(&read) {
        problems.push(e);
    }
    match xpute_packages(&read) {
        Ok(packages) => {
            problems.extend(cycles(&files, &read, &packages));
            match xpute_crates(&read) {
                Ok(crates) => problems.extend(out_of_order(
                    &XPUTE_LAYERS,
                    xpute_uses(&files, &read, &crates, &packages),
                )),
                Err(e) => problems.push(e),
            }
        }
        Err(e) => problems.push(e),
    }
    Ok(problems)
}

fn main() -> ExitCode {
    match run() {
        Ok(problems) if problems.is_empty() => {
            println!("check-rules: the memory's addresses, one rounding, ES modules, the layers' order, xpute's parts' order, the profiler beside it, sys/ under the renderer, the screen's pixels, the crates' module trees and a dependency graph without cycles hold");
            ExitCode::SUCCESS
        }
        Ok(problems) => {
            eprintln!("{}\n\ncheck-rules: {}", problems.join("\n"), problems.len());
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("[check-rules] {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlinked_file_named() {
        let files = [
            "rust/a/src/lib.rs",
            "rust/a/src/one.rs",
            "rust/a/src/one/two.rs",
            "rust/a/src/gen.spec.rs",
            "rust/a/src/stray.rs",
            "rust/a/Cargo.toml",
            "rust/a/src/test_bin.rs",
        ];
        let read = |f: &str| {
            Some(
                match f {
                    "rust/a/src/lib.rs" => {
                        "mod one;
#[path = \"gen.spec.rs\"]
mod spec;"
                    }
                    "rust/a/src/one.rs" => "pub(crate) mod two;",
                    "rust/a/Cargo.toml" => "[[test]]\npath = \"src/test_bin.rs\"",
                    _ => "",
                }
                .to_string(),
            )
        };
        let found = unlinked(&files, &read);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("rust/a/src/stray.rs:"));
    }

    #[test]
    fn order_refuses_up_beside_and_root() {
        const LEVELS: [(&str, u8); 4] = [("top", 2), ("mid", 1), ("f", 1), ("low", 0)];
        let crate_order = Order {
            root: "rust/k/src/",
            path: "crate",
            levels: &LEVELS,
            heads: &[],
            beside: &["perf"],
            own: None,
        };
        const PARTS: [(&str, u8); 2] = [("a", 1), ("b", 0)];
        let folder = Order {
            root: "rust/k/src/domain/f/",
            path: "crate::domain::f",
            levels: &PARTS,
            heads: &[],
            beside: &[],
            own: Some(2),
        };
        let files = [
            "rust/k/src/top.rs",
            "rust/k/src/mid.rs",
            "rust/k/src/low.rs",
            "rust/k/src/perf.rs",
            "rust/k/src/domain/f/a.rs",
            "rust/k/src/domain/f/b.rs",
        ];
        let read = |f: &str| {
            Some(
                match f {
                    "rust/k/src/top.rs" => "use crate::perf::x;\nuse crate::low;",
                    "rust/k/src/mid.rs" => "pub(crate) fn f() {}\nuse crate::perf::y;",
                    "rust/k/src/low.rs" => "use crate::{mid::z, low};",
                    "rust/k/src/perf.rs" => "use crate::top;",
                    "rust/k/src/domain/f/a.rs" => "use crate::domain::f::{b::x, sequence};",
                    "rust/k/src/domain/f/b.rs" => {
                        "use crate::domain::f::sequence;\nuse crate::domain::f;\nuse crate::domain::fx;"
                    }
                    _ => "",
                }
                .to_string(),
            )
        };
        let found = out_of_order(&crate_order, folder_uses(&files, &read, &crate_order));
        let at = |found: &[String]| -> Vec<String> {
            found
                .iter()
                .map(|p| p.split(": ").next().unwrap_or("").to_string())
                .collect()
        };
        assert_eq!(
            at(&found),
            ["rust/k/src/mid.rs:2", "rust/k/src/low.rs:1"],
            "{found:?}"
        );
        let found = out_of_order(&folder, folder_uses(&files, &read, &folder));
        assert_eq!(
            at(&found),
            [
                "rust/k/src/domain/f/a.rs:1",
                "rust/k/src/domain/f/b.rs:1",
                "rust/k/src/domain/f/b.rs:2"
            ],
            "{found:?}"
        );
    }

    /// A layer names the query by neither of its spellings.
    #[test]
    fn layer_names_no_query() {
        let order = ORDERS
            .iter()
            .find(|o| o.root == CORE)
            .expect("the core's order");
        let f = format!("{CORE}domain/layer/place.rs");
        for text in [
            "use crate::domain::query::World;",
            "use crate::domain::layer::query::World;",
        ] {
            let found = out_of_order(
                order,
                folder_uses(&[f.as_str()], &|_| Some(text.to_string()), order),
            );
            assert_eq!(found.len(), 1, "{text}: {found:?}");
        }
    }

    /// Each of pxl-render's orders refuses a name up it, as `ORDERS` holds them: the shaders'
    /// words naming the records, the calls naming the draw lists, a frame's data naming the
    /// device, the shared model pass naming one over it; and no order refuses the name down.
    #[test]
    fn render_orders_refuse_an_edge_up() {
        let up = [
            (
                "frame/shader/word.rs",
                "use crate::frame::shader::record::x;",
                "crate::frame::shader",
            ),
            ("gpu/api.rs", "use crate::gpu::draw::x;", "crate::gpu"),
            (
                "frame/shader/record.rs",
                "use crate::gpu::draw::x;",
                "crate",
            ),
            ("pass/model.rs", "use crate::pass::decor::x;", "crate::pass"),
        ];
        for (file, text, path) in up {
            let o = ORDERS
                .iter()
                .find(|o| o.path == path && o.root.starts_with(RENDER))
                .expect("the order");
            let f = format!("{RENDER}{file}");
            let files = [f.as_str()];
            let found = out_of_order(o, folder_uses(&files, &|_| Some(text.to_string()), o));
            assert_eq!(found.len(), 1, "{file}: {found:?}");
            assert!(found[0].starts_with(&format!("{f}:1:")), "{found:?}");
        }
        let down = ORDERS
            .iter()
            .find(|o| o.path == "crate" && o.root == RENDER)
            .expect("the crate's order");
        let f = format!("{RENDER}renderer.rs");
        let names = "use crate::gpu::draw::x;\nuse crate::pass::mark::y;\nuse crate::ray::z;";
        let found = out_of_order(
            down,
            folder_uses(&[f.as_str()], &|_| Some(names.to_string()), down),
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn client_order_refuses_an_import_up() {
        const LEVELS: [(&str, u8); 5] = [
            ("app", 3),
            ("ui", 2),
            ("runtime", 2),
            ("service", 1),
            ("abi", 0),
        ];
        let order = Order {
            root: "ts/c/src/",
            path: "",
            levels: &LEVELS,
            heads: &[&["ui"], &["runtime"]],
            beside: &[],
            own: None,
        };
        const PARTS: [(&str, u8); 2] = [("core", 1), ("platform", 0)];
        let parts = Order {
            root: "ts/c/src/service/",
            path: "",
            levels: &PARTS,
            heads: &[],
            beside: &[],
            own: None,
        };
        let files = [
            "ts/tsconfig.json",
            "ts/c/src/app.tsx",
            "ts/c/src/ui/view.tsx",
            "ts/c/src/runtime/open.ts",
            "ts/c/src/service/core/core.ts",
            "ts/c/src/service/platform/db.ts",
            "ts/c/src/abi/cmd.spec.ts",
        ];
        let read = |f: &str| {
            Some(
                match f {
                    "ts/tsconfig.json" => {
                        "// The aliases.\n{ \"compilerOptions\": { \"paths\": { \"@/*\": [\"./c/src/*\"], \"@pxl/c/*\": [\"./c/src/*\"] } } }"
                    }
                    "ts/c/src/app.tsx" => "import { App } from \"@/ui/view.tsx\";\nimport \"./runtime/open.ts\";",
                    "ts/c/src/ui/view.tsx" => {
                        "import { k } from \"@/service/core/core.ts\";\n/** as from \"@/runtime/open.ts\" */\nimport { open } from \"../runtime/open.ts\";"
                    }
                    "ts/c/src/runtime/open.ts" => {
                        "import {\n  k,\n} from \"@pxl/c/service/core/core.ts\";\nnew Worker(new URL(\"./worker.ts\", location.href));"
                    }
                    "ts/c/src/service/core/core.ts" => {
                        "import type { View } from \"@pxl/c/ui/view.tsx\";\nimport { db } from \"../platform/db.ts\";\nimport { useState } from \"react\";"
                    }
                    "ts/c/src/service/platform/db.ts" => "import { k } from \"@/service/core/core.ts\";",
                    "ts/c/src/abi/cmd.spec.ts" => "export { k } from \"../service/core/core.ts\";",
                    _ => "",
                }
                .to_string(),
            )
        };
        let at = |o: &Order| -> Vec<String> {
            out_of_order(o, folder_uses(&files, &read, o))
                .iter()
                .map(|p| p.split(": ").next().unwrap_or("").to_string())
                .collect()
        };
        assert_eq!(
            at(&order),
            [
                "ts/c/src/ui/view.tsx:3",
                "ts/c/src/service/core/core.ts:1",
                "ts/c/src/abi/cmd.spec.ts:1"
            ]
        );
        assert_eq!(at(&parts), ["ts/c/src/service/platform/db.ts:1"]);
    }

    /// The entries a device's host may not name are the functions its runtime's C header
    /// declares, a declaration's first name whatever its lines, and no name a comment gives.
    #[test]
    fn runtime_entries_are_its_header_declarations() {
        let header = "// Starts it (`xpute_take`).\nuint32_t xpute_start(const uint32_t *named,\n    size_t named_len);\nvoid xpute_notify_on(void (*notify)(void));\n";
        assert_eq!(declared(header), ["xpute_start", "xpute_notify_on"]);
    }

    /// The two workspaces agree on one `[patch.crates-io]` table entry for entry, its comments,
    /// its blank lines and what follows it aside, or on none.
    #[test]
    fn patch_tables_agree_entry_for_entry() {
        let table = |rev: &str| {
            Some(format!(
                "[workspace]\n\n[patch.crates-io]\n# The fork.\nwgpu = {{ rev = \"{rev}\" }}\n\nnaga = {{ rev = \"{rev}\" }}\n\n[profile.dev]\nopt-level = 1\n"
            ))
        };
        let none = || Some("[workspace]\n".to_string());
        let agree = |rust: Option<String>, xpute: Option<String>| {
            patches_agree(&|f| match f {
                RUST_CARGO => rust.clone(),
                XPUTE_CARGO => xpute.clone(),
                _ => None,
            })
            .is_ok()
        };
        let bare = table("a").map(|t| t.replace("# The fork.\n", "").replace("\n\nnaga", "\nnaga"));
        assert!(agree(table("a"), bare));
        assert!(agree(none(), none()));
        assert!(!agree(table("a"), table("b")));
        assert!(!agree(table("a"), none()));
        assert!(!agree(none(), table("a")));
    }

    #[test]
    fn sys_refuses_renderer() {
        assert_eq!(
            sys_refused("use pxl_spec::x;\nuse pxl_render::gpu::text::Raster;\n// pxl_render"),
            vec![2]
        );
    }

    #[test]
    fn screen_reach_any_name() {
        for (text, at) in [
            ("use crate::domain::view::frame::screen::size;", vec![1]),
            ("use crate::domain::view::frame::{\n    pose::view,\n    screen::units_per_px_at,\n};", vec![1]),
            ("use crate::domain::view::frame::{pose, screen};", vec![1]),
            ("use crate::domain::view::frame::*;\nfn f() { size(); }", vec![1]),
            ("use crate::domain::view::frame as f;", vec![1]),
            ("pub use crate::domain::view::frame::screen::size as px;", vec![1]),
            ("let w = f::screen::size()[0];", vec![1]),
        ] {
            assert_eq!(screen_reached(text), at, "{text}");
        }
        for text in [
            "use crate::domain::view::frame::pose::view;",
            "// a comment naming frame::screen::size",
            "/// The screen as the eye sees it.",
            "let screen_drawn = 1;",
        ] {
            assert!(screen_reached(text).is_empty(), "{text}");
        }
    }
}
