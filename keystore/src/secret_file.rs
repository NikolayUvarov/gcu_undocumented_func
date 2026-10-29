// A program's or a boot service's secrets as the key service keeps them (idl/keystore.wit 1.4, 1.5), before they are
// sealed: a count, then each secret's name (its length, then its bytes) and value (its length, two bytes little-endian,
// then its bytes), in the order they were first kept. secrets.rs seals this under its owner's key; tests/secrets_host.rs
// checks it on the host.
use alloc::vec::Vec;

/// A secret's name: 1 to NAME_MAX letters, digits, `.`, `_` or `-`.
pub const NAME_MAX: usize = 32;
/// A secret's value: 1 to VALUE_MAX bytes.
pub const VALUE_MAX: usize = 16384;
/// The secrets one program keeps at most.
pub const SECRETS: usize = 16;
/// The bytes of one program's names and values together at most, so that its file stays small.
pub const BYTES_MAX: usize = 65536;
/// A program's name as the loader names a task: 1 to PROGRAM_MAX lowercase letters, digits, `.`, `_` or `-`.
pub const PROGRAM_MAX: usize = 16;
/// The longest plain record: the count, then each secret's two lengths beside BYTES_MAX of names and values.
pub const PLAIN_MAX: usize = 1 + SECRETS * 3 + BYTES_MAX;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Refused { Name, Value, Full, Damaged }

/// Whose secrets a file holds: a program's, by the name the loader gave its client, or a boot service's, by the badge
/// init lent it (1.5, 308-KRN-0183). The two never meet: each has its directory and its key's label.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind { Program, Service }

impl Kind {
    pub fn dir(self) -> &'static str { match self { Self::Program => "system/keystore/secrets", Self::Service => "system/keystore/services" } }
    pub fn label(self) -> &'static [u8] { match self { Self::Program => b"MIND keystore: a program's secrets\0", Self::Service => b"MIND keystore: a service's secrets\0" } }
    /// What the file's key is the SHA-256 of: the label, the device key's seed and the name; the caller wipes it.
    pub fn key_material(self, seed: &[u8; 32], name: &str) -> Vec<u8> { [self.label(), seed, name.as_bytes()].concat() }
}

pub fn name_ok(name: &[u8]) -> bool { (1..=NAME_MAX).contains(&name.len()) && name.iter().all(|&b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) }

pub fn program_ok(program: &[u8]) -> bool {
    (1..=PROGRAM_MAX).contains(&program.len()) && program != b"." && program != b".."
        && program.iter().all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// Overwrites a copy of secret material.
pub fn wipe(bytes: &mut [u8]) { for byte in bytes.iter_mut() { unsafe { core::ptr::write_volatile(byte, 0) }; } }

/// One program's secrets; their values are wiped when it is dropped.
#[derive(Default)]
pub struct Secrets { entries: Vec<(Vec<u8>, Vec<u8>)> }

impl Secrets {
    pub fn get(&self, name: &[u8]) -> Option<&[u8]> { self.entries.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_slice()) }

    /// Keeps `value` as `name`, in place of one kept under the name.
    pub fn put(&mut self, name: &[u8], value: &[u8]) -> Result<(), Refused> {
        if !name_ok(name) { return Err(Refused::Name); }
        if value.is_empty() || value.len() > VALUE_MAX { return Err(Refused::Value); }
        let at = self.entries.iter().position(|(n, _)| n == name);
        let others = self.entries.iter().enumerate().filter(|&(i, _)| Some(i) != at).map(|(_, (n, v))| n.len() + v.len()).sum::<usize>();
        if (at.is_none() && self.entries.len() >= SECRETS) || others + name.len() + value.len() > BYTES_MAX { return Err(Refused::Full); }
        match at {
            Some(at) => { wipe(&mut self.entries[at].1); self.entries[at].1 = value.to_vec(); }
            None => self.entries.push((name.to_vec(), value.to_vec())),
        }
        Ok(())
    }

    /// Forgets `name`; false when none was kept.
    pub fn delete(&mut self, name: &[u8]) -> bool {
        let Some(at) = self.entries.iter().position(|(n, _)| n == name) else { return false };
        let (_, mut value) = self.entries.remove(at);
        wipe(&mut value);
        true
    }

    pub fn names(&self) -> impl Iterator<Item = &[u8]> { self.entries.iter().map(|(n, _)| n.as_slice()) }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// The plain record.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + self.entries.iter().map(|(n, v)| 3 + n.len() + v.len()).sum::<usize>());
        out.push(self.entries.len() as u8);
        for (name, value) in &self.entries {
            out.push(name.len() as u8);
            out.extend_from_slice(name);
            out.extend_from_slice(&(value.len() as u16).to_le_bytes());
            out.extend_from_slice(value);
        }
        out
    }

    /// The secrets of a plain record, which must hold nothing else and nothing the limits refuse.
    pub fn decode(plain: &[u8]) -> Result<Self, Refused> {
        let mut secrets = Secrets::default();
        let (&count, mut rest) = plain.split_first().ok_or(Refused::Damaged)?;
        for _ in 0..count {
            let (&len, after) = rest.split_first().ok_or(Refused::Damaged)?;
            let (name, after) = after.split_at_checked(len as usize).ok_or(Refused::Damaged)?;
            let (len, after) = after.split_at_checked(2).ok_or(Refused::Damaged)?;
            let (value, after) = after.split_at_checked(u16::from_le_bytes([len[0], len[1]]) as usize).ok_or(Refused::Damaged)?;
            if secrets.get(name).is_some() { return Err(Refused::Damaged); }
            secrets.put(name, value).map_err(|_| Refused::Damaged)?;
            rest = after;
        }
        if !rest.is_empty() { return Err(Refused::Damaged); }
        Ok(secrets)
    }
}

impl Drop for Secrets {
    fn drop(&mut self) { for (_, value) in self.entries.iter_mut() { wipe(value); } }
}
