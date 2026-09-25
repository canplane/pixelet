// pxl-geo-bake/cli.rs

//! A command's arguments, read the one way every command here reads them: a
//! flag as `--name value` or a bare `--switch`, in any order, and whatever
//! is not a flag handed back as it stands.

use std::str::FromStr;

pub struct Flags {
    argv: Vec<String>,
    at: usize,
}

impl Flags {
    /// The process's own arguments, past the program's name.
    pub fn from_env() -> Flags {
        Flags::of(std::env::args().skip(1).collect())
    }

    pub fn of(argv: Vec<String>) -> Flags {
        Flags { argv, at: 0 }
    }

    /// The next argument: a flag's name or a bare word. None past the last.
    pub fn arg(&mut self) -> Option<String> {
        let arg = self.argv.get(self.at).cloned();
        self.at += 1;
        arg
    }

    /// The value the flag just read carries.
    pub fn value(&mut self) -> Result<String, String> {
        let flag = self.argv[self.at - 1].clone();
        self.arg().ok_or_else(|| format!("{flag} wants a value"))
    }

    /// The value the flag just read carries, as a `T`.
    pub fn parse<T: FromStr>(&mut self) -> Result<T, String> {
        let flag = self.argv[self.at - 1].clone();
        self.value()?.parse().map_err(|_| format!("bad {flag}"))
    }
}
