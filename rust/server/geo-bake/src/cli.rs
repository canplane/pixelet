// pxl-geo-bake/cli.rs

use std::str::FromStr;

/// Enters the checkout a bake or the tile server works in: the nearest
/// directory up from where it was started that holds this crate's manifest,
/// so that `./out`, the sources under `infra/` and `asset/`, and the paths its
/// arguments give are the checkout's wherever it was started, `rust/` under
/// cargo among them. Found where it runs, as pxl-tool's `root` is; started
/// outside a checkout, it stays where it is.
pub fn enter() -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| format!("where this runs: {e}"))?;
    let Some(root) = here
        .ancestors()
        .find(|d| d.join("rust/server/geo-bake/Cargo.toml").is_file())
    else {
        return Ok(());
    };
    std::env::set_current_dir(root).map_err(|e| format!("{}: {e}", root.display()))
}

pub struct Flags {
    argv: Vec<String>,
    at: usize,
}

impl Flags {
    pub fn from_env() -> Flags {
        Flags::of(std::env::args().skip(1).collect())
    }

    pub fn of(argv: Vec<String>) -> Flags {
        Flags { argv, at: 0 }
    }

    pub fn arg(&mut self) -> Option<String> {
        let arg = self.argv.get(self.at).cloned();
        self.at += 1;
        arg
    }

    pub fn val(&mut self) -> Result<String, String> {
        let flag = self.argv[self.at - 1].clone();
        self.arg().ok_or_else(|| format!("{flag} wants a value"))
    }

    pub fn parse<T: FromStr>(&mut self) -> Result<T, String> {
        let flag = self.argv[self.at - 1].clone();
        self.val()?.parse().map_err(|_| format!("bad {flag}"))
    }
}
