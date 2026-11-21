//! The hub's configuration, `hub.conf` in its data directory: the admins' and agents' public keys and the settings it
//! starts with (docs/hub/README.md, 4.4 and 7.1). One entry a line; `#` starts a comment. A line out of this form stops
//! the start, naming it.
//!
//! ```text
//! admin <name> <public key, 64 hex>
//! agent <TRK> <intake|track|integrator> <public key, 64 hex>
//! setting <name> <value>
//! ```
//! The settings the admin changes later are in the journal and win over these.

use crate::hub::{Agent, Keys, Settings};
use improve_proto::state::Role;
use std::path::Path;

pub fn parse(text: &str) -> Result<(Keys, Settings), String> {
    let mut keys = Keys::default();
    let mut settings = Settings::default();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let at = |why: &str| format!("hub.conf, line {}: {why}", n + 1);
        let words: Vec<&str> = line.split_whitespace().collect();
        let key = |w: &str| improve_proto::parse_hex32(w).filter(|k| ed25519_dalek::VerifyingKey::from_bytes(k).is_ok()).ok_or_else(|| at("not an Ed25519 public key in lower-case hex"));
        match words.as_slice() {
            ["admin", name, k] => {
                if name.is_empty() || name.len() > 32 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                    return Err(at("an admin's name is 1 to 32 of A-Z a-z 0-9 - _"));
                }
                let k = key(k)?;
                if keys.admins.values().any(|v| v == name) || keys.admins.insert(k, name.to_string()).is_some() { return Err(at("an admin twice")); }
            }
            ["agent", track, role, k] => {
                if track.len() != 3 || !track.bytes().all(|b| b.is_ascii_uppercase()) { return Err(at("a track's code is three capital letters")); }
                let role = match *role { "intake" => Role::Intake, "track" => Role::Track, "integrator" => Role::Integrator, _ => return Err(at("an agent's role is intake, track or integrator")) };
                if keys.agents.insert(key(k)?, Agent { track: track.to_string(), role }).is_some() { return Err(at("an agent's key twice")); }
            }
            ["setting", name, value] => settings.set(name, value).map_err(|why| at(&why))?,
            _ => return Err(at("not admin NAME KEY, agent TRK ROLE KEY or setting NAME VALUE")),
        }
    }
    for k in keys.admins.keys() { if keys.agents.contains_key(k) { return Err("hub.conf: one key is both an admin's and an agent's".into()); } }
    Ok((keys, settings))
}

pub fn load(path: &Path) -> Result<(Keys, Settings), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text)
}
