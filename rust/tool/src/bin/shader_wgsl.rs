// pxl-tool/bin/shader_wgsl.rs

//! Makes pxl-spec-core's WGSL (pxl-tool shader.rs): rust/client/shader built
//! by rust-gpu inside a Linux container, its SPIR-V read into WGSL by naga
//! (pxl-shader-build), each entry checked in under its header and the digest
//! of the sources that made it.
//!
//!   cargo shader-wgsl            write them   (in rust/)
//!   cargo shader-wgsl --check    fail if one is not current
//!
//! Whether the browser's compiler takes what this writes is the web app's
//! test (ts/app/web src/shader.test.ts, Tint through Dawn), which make
//! test-ts runs: naga takes modules Tint refuses.
//!
//! The check judges only that each checked-in file is what the source makes
//! now; it never compares two implementations, since there is one. Each file
//! carries the digest of its sources, which check_rules recomputes without
//! the container, so a file the source has moved past fails there too.

use std::path::Path;
use std::process::{Command, ExitCode};

use pxl_tool::root;
use pxl_tool::shader::{header, inputs_digest, DIR, ENTRIES};

const IMAGE: &str = "rust:1-bookworm";
const BUILT: &str =
    "rust/target/shader/spirv-builder/spirv-unknown-vulkan1.3/release/deps/pxl_shader.spvs";

/// Where the machine keeps the entries made of each sources' digest, one
/// directory a digest: whatever checkout, export or worktree's sources it
/// was, the same digest is the same bytes, so one made is made for all.
fn made_home() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("no $HOME")?;
    let caches = if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Caches")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".cache"))
    };
    Ok(caches.join("pixelet/shader"))
}

/// An entry, the file it is checked in as, and its module as checked in.
type Module = (&'static str, &'static str, String);

/// Each entry's module as it is checked in, under its header, and the digest
/// of the sources it was made of. Taken from the machine's entries of the
/// sources' digest where it has them; else, or always when `fresh`, built of
/// a copy of the sources (`inputs`), whose digest is the one returned, so
/// what is built is what the digest names, whatever the checkout does
/// meanwhile. The toolchain comes from rust/client/shader/rust-toolchain.toml,
/// which rustup's cargo installs on first use.
fn generate(root: &Path, fresh: bool) -> Result<(String, Vec<Module>), String> {
    let home = made_home()?;
    let mut digest = inputs_digest(root)?;
    if fresh || !home.join(&digest).join("done").is_file() {
        std::fs::create_dir_all(&home).map_err(|e| format!("{}: {e}", home.display()))?;
        let copy = home.join(format!(".copy-{}", std::process::id()));
        let built = snapshot(root, &copy).and_then(|()| {
            digest = inputs_digest(&copy)?;
            run_container(&copy, &home, &digest, fresh)
        });
        let _ = std::fs::remove_dir_all(&copy);
        built?;
    }
    let made = home.join(&digest);
    // The most lately used are kept (`run_container`'s eviction).
    let _ = std::fs::OpenOptions::new()
        .write(true)
        .open(made.join("done"))
        .and_then(|f| f.set_modified(std::time::SystemTime::now()));
    let entries = ENTRIES
        .iter()
        .map(|&(entry, file)| {
            let path = made.join(format!("{entry}.wgsl"));
            let wgsl =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok((entry, file, header() + &wgsl))
        })
        .collect::<Result<_, String>>()?;
    Ok((digest, entries))
}

/// The build's sources, and nothing else, copied from `root` to `copy`.
fn snapshot(root: &Path, copy: &Path) -> Result<(), String> {
    for f in inputs(root)? {
        let to = copy.join(&f);
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::copy(root.join(&f), &to).map_err(|e| format!("{f}: {e}"))?;
    }
    Ok(())
}

/// rust-gpu over `copy`, its entries into `home`'s directory of `digest`.
/// Under the volume's lock: the sources copied to `WORK`, the last run's
/// entries cleared so one rust-gpu did not make cannot be taken, and the
/// directory asked for again, since a run that waited may find the one
/// before it made the same digest. A digest's directory is renamed into place
/// whole and never replaced: a fresh build that differs from it fails, as a
/// digest that names two outputs.
fn run_container(copy: &Path, home: &Path, digest: &str, fresh: bool) -> Result<(), String> {
    let make = format!(
        "set -e
made=/made/{digest}
if [ {fresh} = true ] || [ ! -f $made/done ]; then
  rm -rf {WORK}
  cp -R /copy {WORK}
  rm -rf {BUILT}
  cd {WORK}/rust/client/shader
  cargo run -q --release --locked -p pxl-shader-build
  rm -rf /made/.part
  mkdir /made/.part
  cp {BUILT}/*.wgsl /made/.part/
  if [ -f $made/done ]; then
    if ! diff -r -x done /made/.part $made >/dev/null; then
      echo \"the build differs from what the machine made of {digest} before\" >&2
      exit 1
    fi
    rm -rf /made/.part
  else
    touch /made/.part/done
    rm -rf $made
    mv /made/.part $made
  fi
fi
ls -1t /made/*/done | tail -n +{} | while read f; do rm -rf \"${{f%/done}}\"; done",
        KEPT + 1
    );
    let status = Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "-v"])
        .arg(format!("{}:/w", root.display()))
        .args([
            "-w",
            "/w/rust/client/shader",
            "-e",
            "CARGO_TARGET_DIR=/w/rust/target/shader",
            "-e",
            "CARGO_HOME=/w/rust/target/shader/.cargo",
            "-e",
            "RUSTUP_HOME=/w/rust/target/shader/.rustup",
            IMAGE,
            "bash",
            "-c",
            "cargo run -q --release -p pxl-shader-build",
        ])
        .status()
        .map_err(|e| format!("docker: {e}"))?;
    if !status.success() {
        return Err("the rust-gpu build failed".into());
    }
    ENTRIES
        .iter()
        .map(|&(entry, file)| {
            let path = root.join(BUILT).join(format!("{entry}.wgsl"));
            let wgsl =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok((entry, file, header(digest) + &wgsl))
        })
        .collect()
}

fn run() -> Result<bool, String> {
    let check = std::env::args().any(|a| a == "--check");
    let root = root();
    let made = generate(&root, &inputs_digest(&root)?)?;
    let mut current = true;
    for (_, file, text) in &made {
        let target = root.join(DIR).join(file);
        if check {
            if std::fs::read_to_string(&target).ok().as_deref() == Some(text.as_str()) {
                println!("current: {}", target.display());
            } else {
                eprintln!(
                    "stale: {} is not what the source makes now; run `cargo shader-wgsl`",
                    target.display()
                );
                current = false;
            }
        } else {
            std::fs::write(&target, text).map_err(|e| format!("{}: {e}", target.display()))?;
            println!("wrote {} ({} bytes)", target.display(), text.len());
        }
    }
    Ok(current)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("[shader-wgsl] {e}");
            ExitCode::FAILURE
        }
    }
}
