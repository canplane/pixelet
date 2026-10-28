// pxl-tool/bin/shader_wgsl.rs

//! Makes pxl-spec-core's WGSL (pxl-tool shader.rs): rust/client/shader built
//! by rust-gpu inside a Linux container, its SPIR-V read into WGSL by naga
//! (pxl-shader-build), each entry checked in under its header and the digest
//! of the sources that made them in one stamp beside them (pxl-tool
//! shader.rs `STAMP`).
//!
//!   cargo shader-wgsl            write them   (in rust/)
//!   cargo shader-wgsl --force    write them, made again whatever they name
//!   cargo shader-wgsl --check    fail if one is not current
//!
//! A write makes nothing when the stamp already names the digest of what
//! makes the entries as it is now (pxl-tool shader.rs `inputs_digest`), the
//! record check_rules holds them to, so the container would write the same bytes.
//! A digest the machine has made before, from any checkout, export or
//! worktree, is copied from what it made (`made_home`) with no container;
//! one it has not is built in the one volume all of them share. A file
//! edited by hand under its digest is what `--force` is for; `--check`
//! always builds them and compares.
//!
//! Whether the browser's compiler takes what this writes is the web app's
//! test (ts/app/web src/shader.test.ts, Tint through Dawn), which make
//! test-ts runs: naga takes modules Tint refuses.
//!
//! The check judges only that each checked-in file is what the source makes
//! now; it never compares two implementations, since there is one. The
//! set's stamp names the digest of its sources, which check_rules recomputes
//! without the container, so a set the source has moved past fails there too.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use pxl_tool::root;
use pxl_tool::shader::{header, inputs, inputs_digest, made_from, stale, DIR, ENTRIES, STAMP};

/// The image rustup runs in, by its digest: its compiler builds SPIRV-Tools
/// (rustc_codegen_spirv's `use-compiled-tools`), so a tag that moved could
/// move the output.
const IMAGE: &str = "rust@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6";
/// The machine's one home for the build: the toolchain, the crates and the
/// target, a Docker volume rather than any checkout's directory, so a
/// checkout, an export or a worktree each compiles only what its sources
/// change.
const HOME: &str = "pxl-shader";
/// Where a run's sources are built, inside the volume: the same path for
/// every checkout, so the target's dependencies carry over, and copied in
/// anew under the lock, so cargo, which judges a path crate by its files'
/// times, never takes another run's build for this one's.
const WORK: &str = "/cache/w";
const BUILT: &str =
    "/cache/target/spirv-builder/spirv-unknown-vulkan1.3/release/deps/pxl_shader.spvs";
/// How many sources' entries the machine keeps made, the most lately used.
const KEPT: usize = 100;

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

/// Each entry's module as it is checked in, under its header, and the digest
/// of the sources it was made of. Taken from the machine's entries of the
/// sources' digest where it has them; else, or always when `fresh`, built of
/// a copy of the sources (`inputs`), whose digest is the one returned, so
/// what is built is what the digest names, whatever the checkout does
/// meanwhile. The toolchain comes from rust/client/shader/rust-toolchain.toml,
/// which rustup's cargo installs on first use.
fn generate(
    root: &Path,
    fresh: bool,
) -> Result<(String, Vec<(&'static str, &'static str, String)>), String> {
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
        .args(["run", "--rm", "--init", "--platform", "linux/arm64"])
        .arg("-v")
        .arg(format!("{}:/copy:ro", copy.display()))
        .arg("-v")
        .arg(format!("{HOME}:/cache"))
        .arg("-v")
        .arg(format!("{}:/made", home.display()))
        .args([
            "-e",
            "CARGO_TARGET_DIR=/cache/target",
            "-e",
            "CARGO_HOME=/cache/cargo",
            "-e",
            "RUSTUP_HOME=/cache/rustup",
            IMAGE,
            "flock",
            "/cache/lock",
            "sh",
            "-c",
        ])
        .arg(make)
        .status()
        .map_err(|e| format!("docker: {e}"))?;
    if !status.success() {
        return Err("the container's run failed".into());
    }
    Ok(())
}

fn run() -> Result<bool, String> {
    let check = std::env::args().any(|a| a == "--check");
    let force = std::env::args().any(|a| a == "--force");
    let root = root();
    let digest = inputs_digest(&root)?;
    if !check && !force && stale(&root, &digest).is_empty() {
        println!("current: the entries were made from the sources' digest {digest}");
        return Ok(true);
    }
    let (built, made) = generate(&root, check || force)?;
    let stamp = root.join(DIR).join(STAMP);
    if check {
        let named = made_from(&root).as_deref() == Some(built.as_str());
        let mut current = named;
        for (_, file, text) in &made {
            let target = root.join(DIR).join(file);
            if std::fs::read_to_string(&target).ok().as_deref() != Some(text.as_str()) {
                // Made from these sources by the stamp, yet not what they
                // make: an entry edited by hand.
                let mend = if named { " --force" } else { "" };
                eprintln!(
                    "stale: {} is not what the source makes now; run `cargo shader-wgsl{mend}`",
                    target.display()
                );
                current = false;
            }
        }
        if !named {
            eprintln!(
                "stale: {} names other sources; run `cargo shader-wgsl`",
                stamp.display()
            );
        }
        return Ok(current);
    }
    // The stamp is taken away first and put back last, each entry renamed
    // into place between: a run cut short leaves no stamp, so the set reads
    // as stale whatever the sources go back to. An entry already what it
    // makes is left alone, so a build or a watcher that reads it is not woken.
    let _ = std::fs::remove_file(&stamp);
    for (_, file, text) in &made {
        let target = root.join(DIR).join(file);
        if std::fs::read_to_string(&target).ok().as_deref() != Some(text.as_str()) {
            replace(&target, text)?;
            println!("wrote {} ({} bytes)", target.display(), text.len());
        }
    }
    replace(&stamp, &format!("{built}\n"))?;
    if built != digest {
        eprintln!("the sources changed while they were made; they read as stale until made again");
    }
    Ok(true)
}

/// `text` renamed into place at `target` from a name this run alone writes.
fn replace(target: &Path, text: &str) -> Result<(), String> {
    let part = target.with_extension(format!("part-{}", std::process::id()));
    std::fs::write(&part, text)
        .and_then(|()| std::fs::rename(&part, target))
        .map_err(|e| format!("{}: {e}", target.display()))
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
