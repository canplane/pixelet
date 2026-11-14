// pxl-tool/spec/setting.rs

//! The settings, each by the number `schema.json5` gives it, and a choice's
//! options by theirs; a number is given out once in its table. The core's
//! table carries the numbers and no names, which are the UI's, for its
//! rows and the locale. The UI gets a separate production table without
//! the `dev` settings so the bundler drops them.
//! A setting whose `stored` is false the core's store neither keeps nor reads.

use xpute_gen::spec::data::{table_outputs, ts_literal, DataSpec};

use super::{
    description, file, js_num, marks, pascal, read, rs_field, rs_member, u32_of, Node, Output,
};
use crate::root;

const JSON: &str = "spec/client/core/domain/setting/schema.json5";

struct Setting<'a> {
    key: &'a str,
    spec: &'a Node,
    /// Offered by a development build only (`"dev": true`).
    dev: bool,
    /// Kept by the core's store unless marked `"stored": false`.
    stored: bool,
}

impl Setting<'_> {
    fn kind(&self) -> &str {
        self.spec.str_at("kind").unwrap_or_default()
    }
    fn options(&self) -> Option<&str> {
        self.spec.str_at("options")
    }
    fn default(&self) -> &Node {
        self.spec
            .get("default")
            .map(Node::val)
            .unwrap_or(&Node::Null)
    }
}

struct List<'a> {
    name: &'a str,
    /// Each option and its number, in the JSON's order.
    vals: Vec<(String, u32)>,
    doc: Option<Vec<String>>,
}

/// A number as `schema.json5` gives one, a u32.
fn number(n: Option<&Node>, what: &str) -> Result<u32, String> {
    n.and_then(u32_of)
        .ok_or(format!("{JSON}: {what} has no number"))
}

/// A setting's bool field, `default` where it is absent; anything else there is refused.
fn bool_at(spec: &Node, key: &str, name: &str, default: bool) -> Result<bool, String> {
    match spec.get(name) {
        None => Ok(default),
        Some(Node::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("{JSON}: {key}'s {name} is not a bool")),
    }
}

/// A table's numbers are each given out once.
fn check_numbers(table: &str, numbered: &[(&str, u32)]) -> Result<(), String> {
    for (i, (name, n)) in numbered.iter().enumerate() {
        if let Some((other, _)) = numbered[..i].iter().find(|(_, m)| m == n) {
            return Err(format!("{JSON}: {table}'s {name} and {other} are both {n}"));
        }
    }
    Ok(())
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let spec = read(JSON)?;
    let every = spec.get("SETTING").ok_or(format!("{JSON}: no SETTING"))?;
    let settings: Vec<Setting> = every
        .named()
        .map(|(key, spec)| {
            Ok(Setting {
                key,
                spec,
                dev: bool_at(spec, key, "dev", false)?,
                stored: bool_at(spec, key, "stored", true)?,
            })
        })
        .collect::<Result<_, String>>()?;
    let numbers = settings
        .iter()
        .map(|s| Ok((s.key, number(s.spec.get("number"), s.key)?)))
        .collect::<Result<Vec<_>, String>>()?;
    check_numbers("SETTING", &numbers)?;
    let number_of = |s: &Setting| {
        numbers
            .iter()
            .find(|(k, _)| *k == s.key)
            .map(|(_, n)| *n)
            .unwrap_or_default()
    };
    let mut lists: Vec<List> = Vec::new();
    for (name, n) in spec.named() {
        if name == "SETTING" {
            continue;
        }
        let vals = n
            .is_wrapper()
            .then(|| n.val())
            .filter(|v| v.is_obj())
            .ok_or(format!(
                "{JSON}: {name} is not a table of options by number"
            ))?;
        let vals = vals
            .named()
            .map(|(o, v)| Ok((o.to_string(), number(Some(v), &format!("{name}'s {o}"))?)))
            .collect::<Result<Vec<_>, String>>()?;
        check_numbers(
            name,
            &vals
                .iter()
                .map(|(o, v)| (o.as_str(), *v))
                .collect::<Vec<_>>(),
        )?;
        lists.push(List {
            name,
            vals,
            doc: n.doc(),
        });
    }
    let list = |name: &str| lists.iter().find(|l| l.name == name);
    let options_of = |s: &Setting| {
        s.options()
            .and_then(list)
            .map(|l| l.vals.clone())
            .ok_or(format!("{JSON}: {}'s options name no list", s.key))
    };
    let mut choices: Vec<&str> = Vec::new();
    for s in settings.iter().filter(|s| s.kind() == "choice") {
        let o = s.options().unwrap_or_default();
        if !choices.contains(&o) {
            choices.push(o);
        }
    }

    // The choices' options and the settings, each an enum by its numbers,
    // written by xpute's generator from a table built here.
    let mut table = vec![(
        "$description".to_string(),
        description(&spec.doc().unwrap_or_default()),
    )];
    let enum_entry = |doc: &[String], members: Vec<(String, Node)>| {
        let mut e = vec![
            ("$type".to_string(), Node::Str("enum".into())),
            marks(vec![("repr".to_string(), Node::Str("u32".into()))]),
            ("$description".to_string(), description(doc)),
        ];
        e.extend(members);
        Node::Obj(e)
    };
    for name in &choices {
        let List { vals, doc, .. } = list(name).ok_or(format!("{JSON}: no list {name}"))?;
        let members = vals
            .iter()
            .map(|(o, n)| (o.clone(), Node::Num(f64::from(*n))))
            .collect();
        table.push((
            name.to_string(),
            enum_entry(doc.as_deref().unwrap_or_default(), members),
        ));
    }
    let members = settings
        .iter()
        .map(|s| {
            let mut m = vec![marks(vec![(
                "number".to_string(),
                Node::Num(f64::from(number_of(s))),
            )])];
            m.extend(
                s.spec
                    .get("$description")
                    .map(|d| ("$description".to_string(), d.clone())),
            );
            (s.key.to_string(), Node::Obj(m))
        })
        .collect();
    table.push((
        "SETTING_KEY".into(),
        enum_entry(
            &["Every setting, by its number in schema.json5's SETTING.".into()],
            members,
        ),
    ));
    let d = DataSpec {
        json: JSON,
        ts: Some("ts/client/src/abi/schema.spec.ts"),
        rs: Some("rust/client/core/src/spec/schema.spec.rs"),
        ..DataSpec::DEFAULT
    };
    let mut outs = table_outputs(&root(), &d, Node::Obj(table), &[])?;

    let mut rs: Vec<String> = vec![String::new()];
    let keys_where = |test: &dyn Fn(&Setting) -> bool| {
        let keys: Vec<String> = settings
            .iter()
            .filter(|s| test(s))
            .map(|s| format!("SettingKey::{}", rs_member(s.key)))
            .collect();
        if keys.is_empty() {
            "false".to_string()
        } else {
            format!("matches!(self, {})", keys.join(" | "))
        }
    };
    rs.extend([
        "impl SettingKey {".to_string(),
        "    /// One past the highest number: the length of a list of every".into(),
        "    /// setting's value at its number, where a number not given out stays 0.".into(),
        format!(
            "    pub const SLOT_CNT: usize = {};",
            numbers
                .iter()
                .map(|(_, n)| *n as usize + 1)
                .max()
                .unwrap_or(0)
        ),
    ]);
    let session: Vec<String> = settings
        .iter()
        .filter(|s| !s.stored)
        .map(|s| format!("SettingKey::{}", rs_member(s.key)))
        .collect();
    rs.extend([
        String::new(),
        "    /// Written to the settings' storage file and read from it; one schema.json5".into(),
        "    /// marks `stored` false is held for the session alone.".into(),
        "    pub fn is_stored(self) -> bool {".into(),
        if session.is_empty() {
            "        true".to_string()
        } else {
            format!("        !matches!(self, {})", session.join(" | "))
        },
        "    }".into(),
        String::new(),
        "    /// Offered by a development build only.".into(),
        "    pub fn dev(self) -> bool {".into(),
        format!("        {}", keys_where(&|s| s.dev)),
        "    }".into(),
        String::new(),
        "    /// A choice's option a development build only offers.".into(),
    ]);
    let mut dev_arms = Vec::new();
    for s in settings
        .iter()
        .filter(|s| s.spec.get("dev_options").is_some())
    {
        let names = options_of(s)?;
        let dev: Vec<String> = s
            .spec
            .get("dev_options")
            .and_then(Node::arr)
            .unwrap_or_default()
            .iter()
            .filter_map(|n| n.str())
            .map(|n| {
                names
                    .iter()
                    .find(|(o, _)| o == n)
                    .map(|(_, number)| number.to_string())
                    .ok_or(format!(
                        "{JSON}: {}'s dev option {n} is not one of its options",
                        s.key
                    ))
            })
            .collect::<Result<_, _>>()?;
        dev_arms.push(format!(
            "            SettingKey::{} => matches!(val, {}),",
            rs_member(s.key),
            dev.join(" | ")
        ));
    }
    if dev_arms.is_empty() {
        rs.extend([
            "    pub fn dev_option(self, _val: u32) -> bool {".to_string(),
            "        false".into(),
        ]);
    } else {
        rs.extend([
            "    pub fn dev_option(self, val: u32) -> bool {".to_string(),
            "        match self {".into(),
        ]);
        rs.extend(dev_arms);
        rs.extend(["            _ => false,".to_string(), "        }".into()]);
    }
    rs.extend(["    }".to_string(), "}".into()]);
    let field_type = |s: &Setting| -> Result<String, String> {
        match s.kind() {
            "toggle" => Ok("bool".to_string()),
            "choice" => Ok(pascal(s.options().unwrap_or_default())),
            "index" => {
                let d = s
                    .spec
                    .get("default")
                    .ok_or(format!("{JSON}: {} has no default", s.key))?;
                match d.is_wrapper().then(|| d.str_at("$type")).flatten() {
                    Some(t @ ("u8" | "u16" | "u32")) => Ok(t.to_string()),
                    _ => Err(format!("{JSON}: {} is an index with no width", s.key)),
                }
            }
            kind => Err(format!(
                "{JSON}: {} is a setting of no kind there is: {kind}",
                s.key
            )),
        }
    };
    let default_lit = |s: &Setting| -> Result<String, String> {
        let d = s.default();
        if s.kind() == "choice" {
            let name = d.str().unwrap_or_default();
            if !options_of(s)?.iter().any(|(o, _)| o == name) {
                return Err(format!(
                    "{JSON}: {}'s default is not one of its options",
                    s.key
                ));
            }
            return Ok(format!(
                "{}::{}",
                pascal(s.options().unwrap_or_default()),
                rs_member(name)
            ));
        }
        Ok(match d {
            Node::Bool(b) => b.to_string(),
            Node::Num(n) => js_num(*n),
            Node::Str(s) => s.clone(),
            other => format!("{other:?}"),
        })
    };
    rs.extend(
        [
            "",
            "/// Every setting's value: what the store holds, and what the app reads.",
            "#[derive(Clone, Debug, PartialEq)]",
            "pub struct Setting {",
        ]
        .map(String::from),
    );
    for s in &settings {
        rs.push(format!("    pub {}: {},", rs_field(s.key), field_type(s)?));
    }
    rs.extend(
        [
            "}",
            "",
            "impl Setting {",
            "    /// Every setting at schema.json5's default.",
            "    pub const DEFAULT: Setting = Setting {",
        ]
        .map(String::from),
    );
    for s in &settings {
        rs.push(format!("        {}: {},", rs_field(s.key), default_lit(s)?));
    }
    rs.extend(["    };".to_string(), String::new()]);
    rs.extend(
        [
            "    /// A setting's value as it crosses and is kept: a toggle's 0 or 1, a",
            "    /// choice's option by its number, an index as itself.",
            "    pub fn get(&self, key: SettingKey) -> u32 {",
            "        match key {",
        ]
        .map(String::from),
    );
    rs.extend(settings.iter().map(|s| {
        format!(
            "            SettingKey::{} => self.{} as u32,",
            rs_member(s.key),
            rs_field(s.key)
        )
    }));
    rs.extend(
        [
            "        }",
            "    }",
            "",
            "    /// Sets a setting from its value as it crosses; false, and nothing set,",
            "    /// for a value the setting cannot hold.",
            "    pub fn set(&mut self, key: SettingKey, val: u32) -> bool {",
            "        match key {",
        ]
        .map(String::from),
    );
    for s in &settings {
        let (k, v) = (rs_field(s.key), rs_member(s.key));
        rs.push(match s.kind() {
            "toggle" => format!("            SettingKey::{v} if val <= 1 => self.{k} = val == 1,"),
            "choice" => format!(
                "            SettingKey::{v} => match {}::of(val) {{ Some(o) => self.{k} = o, None => return false }},",
                pascal(s.options().unwrap_or_default())
            ),
            _ => format!(
                "            SettingKey::{v} => match {}::try_from(val) {{ Ok(n) => self.{k} = n, Err(_) => return false }},",
                field_type(s)?
            ),
        });
    }
    rs.extend(
        [
            "            _ => return false,",
            "        }",
            "        true",
            "    }",
            "}",
        ]
        .map(String::from),
    );

    let row = |s: &Setting| -> Result<Node, String> {
        let mut out: Vec<(String, Node)> = vec![(
            "key".to_string(),
            Node::Obj(vec![
                ("$value".to_string(), Node::Num(number_of(s) as f64)),
                ("$type".to_string(), Node::Str("u32".to_string())),
            ]),
        )];
        if let Some(doc) = s.spec.get("$description") {
            out.push(("$description".to_string(), doc.clone()));
        }
        for (f, v) in s.spec.entries() {
            if f == "$description" || f == "number" {
                continue;
            }
            // A field's value without its `$description`, which stays the schema's.
            let v = match v {
                Node::Obj(t) if v.is_wrapper() => Node::Obj(
                    t.iter()
                        .filter(|(k, _)| k != "$description")
                        .cloned()
                        .collect(),
                ),
                v => v.clone(),
            };
            let val = match f.as_str() {
                "options" => Node::Obj(
                    std::iter::once(("$type".to_string(), Node::Str("u32".into())))
                        .chain(
                            options_of(s)?
                                .into_iter()
                                .map(|(o, n)| (o, Node::Num(f64::from(n)))),
                        )
                        .collect(),
                ),
                _ => v,
            };
            match out.iter_mut().find(|(k, _)| k == f) {
                Some(slot) => slot.1 = val,
                None => out.push((f.clone(), val)),
            }
        }
        Ok(Node::Obj(out))
    };
    let table = |keep: &dyn Fn(&Setting) -> bool| -> Result<Node, String> {
        let mut out = Vec::new();
        for s in settings.iter().filter(|s| keep(s)) {
            out.push((s.key.to_string(), row(s)?));
        }
        Ok(Node::Obj(out))
    };
    let mut ts: Vec<String> = Vec::new();
    ts.extend([
        String::new(),
        format!(
            "export const SETTING = {} as const;",
            ts_literal(table(&|_| true)?, "")?
        ),
    ]);
    ts.extend([
        String::new(),
        "/** SETTING without its `dev` entries: what a production bundle carries in its place. */"
            .into(),
        format!(
            "export const SETTING_PROD = {} as const;",
            ts_literal(table(&|s| !s.dev)?, "")?
        ),
    ]);

    for o in &mut outs {
        let tail = if o.path.ends_with(".rs") { &rs } else { &ts };
        o.text = format!("{}{}", o.text, file(tail));
    }
    Ok(outs)
}
