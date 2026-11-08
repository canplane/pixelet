//! The protocols the harness speaks, read from the JSON they are generated from: the UI's
//! (ui.json), xpute's own (xpute/spec/abi/sys.json, gpu.json, fs.json) and the core's
//! files' roots (spec/client/core/sys/io/file.json). The harness names a command, a field,
//! an enum's member or a setting as the spec names it, so none of their numbers is written
//! twice.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// Parses a JSON file of the repository.
pub fn json(path: &str) -> Value {
    let at = Path::new(env!("PXL_REPO")).join(path);
    let text = std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("{}: {e}", at.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", at.display()))
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub optional: bool,
}

#[derive(Clone, Debug)]
pub struct Command {
    pub val: u32,
    pub fields: Vec<Field>,
    /// The fields a completion of the command carries, when the spec gives any.
    pub reply: Vec<Field>,
}

/// A command spec's commands in both directions, its enums' members and its records.
pub struct Protocol {
    commands: BTreeMap<String, Command>,
    enums: BTreeMap<String, BTreeMap<String, u32>>,
    records: BTreeMap<String, Vec<Field>>,
}

fn fields(v: &Value) -> Vec<Field> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|f| Field {
                    name: f["name"].as_str().expect("a field's name").to_string(),
                    optional: f["optional"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Protocol {
    pub fn load(path: &str) -> Protocol {
        let j = json(path);
        let mut commands = BTreeMap::new();
        for side in ["export", "import"] {
            for c in j[side].as_array().into_iter().flatten() {
                let name = c["name"].as_str().expect("a command's name").to_string();
                let val = c["val"].as_u64().expect("a command's number") as u32;
                commands.insert(
                    name,
                    Command {
                        val,
                        fields: fields(&c["pkt"]),
                        reply: fields(&c["reply"]),
                    },
                );
            }
        }
        let mut enums = BTreeMap::new();
        for e in j["enums"].as_array().into_iter().flatten() {
            // An entry without a name and a value is a section heading, not a member.
            let members = e["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| Some((m["name"].as_str()?.to_string(), m["val"].as_u64()? as u32)))
                .collect();
            enums.insert(
                e["name"].as_str().expect("an enum's name").to_string(),
                members,
            );
        }
        let records = j["records"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                let name = r["name"].as_str().expect("a record's name").to_string();
                (name, fields(&r["fields"]))
            })
            .collect();
        Protocol {
            commands,
            enums,
            records,
        }
    }

    /// The protocols at `paths` merged into one. Where two name the same enum, the last
    /// one's wins.
    pub fn load_each(paths: &[&str]) -> Protocol {
        let mut all = Protocol::load(paths[0]);
        for path in &paths[1..] {
            let p = Protocol::load(path);
            all.commands.extend(p.commands);
            all.enums.extend(p.enums);
            all.records.extend(p.records);
        }
        all
    }

    /// The command `name`, if this protocol gives it.
    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands.get(name)
    }

    pub fn command(&self, name: &str) -> &Command {
        self.commands
            .get(name)
            .unwrap_or_else(|| panic!("no command {name}"))
    }

    /// The command's number.
    pub fn number(&self, name: &str) -> u32 {
        self.command(name).val
    }

    /// The name of the command numbered `number`, if this protocol gives it.
    pub fn name_of(&self, number: u32) -> Option<&str> {
        self.commands
            .iter()
            .find(|(_, c)| c.val == number)
            .map(|(n, _)| n.as_str())
    }

    pub fn member(&self, of: &str, name: &str) -> u32 {
        self.enums
            .get(of)
            .and_then(|e| e.get(name))
            .copied()
            .unwrap_or_else(|| panic!("no {of}::{name}"))
    }

    /// The index of `field` among the children of `command`'s packet.
    pub fn at(&self, command: &str, field: &str) -> u32 {
        index(&self.command(command).fields, field)
            .unwrap_or_else(|| panic!("{command} has no field {field}"))
    }

    /// The index of `field` among the children of `record`.
    pub fn record_at(&self, record: &str, field: &str) -> u32 {
        self.records
            .get(record)
            .and_then(|f| index(f, field))
            .unwrap_or_else(|| panic!("{record} has no field {field}"))
    }
}

fn index(fields: &[Field], name: &str) -> Option<u32> {
    fields.iter().position(|f| f.name == name).map(|i| i as u32)
}

/// A field's value as a packet carries it.
#[derive(Clone, Debug)]
pub enum V {
    Num(f64),
    Bool(bool),
    Nums(Vec<u32>),
    Strs(Vec<String>),
}

/// Writes the packet of `command` into `buf`, taking each field from `values` by name and
/// writing an optional field left out as absent. Returns the packet's length, or None for a
/// command that carries no packet.
pub fn write(command: &Command, values: &[(&str, V)], buf: &mut [u8]) -> Option<u32> {
    for (name, _) in values {
        assert!(
            command.fields.iter().any(|f| f.name == *name),
            "no field {name} in {:?}",
            command.fields
        );
    }
    if command.fields.is_empty() {
        return None;
    }
    let mut w = PacketWriter::new(buf, command.fields.len() as u32);
    for f in &command.fields {
        match values.iter().find(|(n, _)| *n == f.name).map(|(_, v)| v) {
            None => {
                assert!(f.optional, "{} is not optional", f.name);
                w.nil();
            }
            Some(V::Num(n)) => {
                w.f64(Some(*n));
            }
            Some(V::Bool(b)) => {
                w.bool(Some(*b));
            }
            Some(V::Nums(a)) => {
                w.u32_array(Some(a));
            }
            Some(V::Strs(a)) => {
                w.strs(Some(a));
            }
        }
    }
    Some(w.finish().expect("the packet fits a slot"))
}

/// The children of a packet's root branch.
pub fn read(pkt: &[u8]) -> BranchCursor<'_> {
    TreeReader::new(pkt)
        .and_then(|r| r.read_branch())
        .expect("a packet is a branch")
}

pub fn num(c: Cursor<'_>) -> f64 {
    c.get_number().expect("a number")
}

pub fn flag(c: Cursor<'_>) -> bool {
    c.get_bool().expect("a bool")
}

/// A setting's number by its key (spec/client/core/domain/view/setting/schema.json).
pub fn setting(key: &str) -> u32 {
    let j = json("spec/client/core/domain/view/setting/schema.json");
    j["SETTING"][key]["number"]
        .as_u64()
        .unwrap_or_else(|| panic!("no setting {key}")) as u32
}

/// A value of the core's files' spec (spec/client/core/sys/io/file.json), by its path.
pub fn file_val(path: &[&str]) -> u32 {
    let mut j = json("spec/client/core/sys/io/file.json");
    for p in path {
        j = j[*p].take();
    }
    j["$val"]
        .as_u64()
        .unwrap_or_else(|| panic!("no value at {path:?}")) as u32
}
