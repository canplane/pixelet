// pxl-geo-bake/cli.rs

use std::str::FromStr;

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

    pub fn value(&mut self) -> Result<String, String> {
        let flag = self.argv[self.at - 1].clone();
        self.arg().ok_or_else(|| format!("{flag} wants a value"))
    }

    pub fn parse<T: FromStr>(&mut self) -> Result<T, String> {
        let flag = self.argv[self.at - 1].clone();
        self.value()?.parse().map_err(|_| format!("bad {flag}"))
    }
}
