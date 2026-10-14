//! The WPA2-PSK networks the key service remembers (550-NET-0024): each network's name and PMK, wrapped together
//! (RFC 3394, AES-256) under a key the service derives from its device key, so the file opens only where that key does
//! and a name cannot be moved onto another network's PMK. The file: `MAGIC`, a count, then one wrapped record each.
use aes_kw::KekAes256;
use crate::Error;

/// The networks kept at most.
pub const NETWORKS: usize = 16;
pub const MAGIC: &[u8; 8] = b"MINDWIF1";
/// A network's record before wrapping: the name's length, the name padded to 32 bytes and 7 zero bytes, the PMK.
const PLAIN: usize = 1 + 32 + 7 + 32;
/// A record wrapped: the plain record and the key wrap's 8-byte integrity value.
pub const RECORD: usize = PLAIN + 8;
/// The longest file.
pub const FILE_MAX: usize = MAGIC.len() + 1 + NETWORKS * RECORD;

#[derive(Clone, Copy)]
struct Network { name: [u8; 32], len: u8, pmk: [u8; 32] }

impl Network {
    fn name(&self) -> &[u8] { &self.name[..self.len as usize] }
}

/// The networks kept, in the order they were first kept.
#[derive(Default)]
pub struct Kept { networks: [Option<Network>; NETWORKS] }

impl Kept {
    /// The PMK kept for network `ssid`.
    pub fn get(&self, ssid: &[u8]) -> Option<&[u8; 32]> { self.networks.iter().flatten().find(|n| n.name() == ssid).map(|n| &n.pmk) }

    /// Keeps `pmk` for network `ssid` in place of one kept for the name: `Space` when NETWORKS others are kept.
    pub fn put(&mut self, ssid: &[u8], pmk: &[u8; 32]) -> Result<(), Error> {
        if ssid.is_empty() || ssid.len() > 32 { return Err(Error::Malformed); }
        let mut name = [0u8; 32];
        name[..ssid.len()].copy_from_slice(ssid);
        let network = Network { name, len: ssid.len() as u8, pmk: *pmk };
        let at = self.networks.iter().position(|n| n.is_some_and(|n| n.name() == ssid))
            .or_else(|| self.networks.iter().position(Option::is_none)).ok_or(Error::Space)?;
        self.networks[at] = Some(network);
        Ok(())
    }

    /// Forgets network `ssid`; false when none was kept.
    pub fn remove(&mut self, ssid: &[u8]) -> bool {
        let Some(at) = self.networks.iter().position(|n| n.is_some_and(|n| n.name() == ssid)) else { return false };
        if let Some(network) = self.networks[at].as_mut() { network.pmk.fill(0); }
        self.networks[at] = None;
        true
    }

    /// The names kept.
    pub fn names(&self) -> impl Iterator<Item = &[u8]> { self.networks.iter().flatten().map(Network::name) }

    /// The file, under `kek`; its length in `out`.
    pub fn encode(&self, kek: &[u8; 32], out: &mut [u8; FILE_MAX]) -> usize {
        let wrap = KekAes256::from(*kek);
        out[..MAGIC.len()].copy_from_slice(MAGIC);
        let mut at = MAGIC.len() + 1;
        let mut count = 0u8;
        for network in self.networks.iter().flatten() {
            let mut plain = [0u8; PLAIN];
            plain[0] = network.len;
            plain[1..33].copy_from_slice(&network.name);
            plain[40..].copy_from_slice(&network.pmk);
            // Wrapping 72 bytes into 80 cannot fail.
            let _ = wrap.wrap(&plain, &mut out[at..at + RECORD]);
            plain.fill(0);
            at += RECORD;
            count += 1;
        }
        out[MAGIC.len()] = count;
        at
    }

    /// The networks of a file written by `encode` under `kek`: `KeyData` when a record does not unwrap under it (another
    /// key, or a changed byte), `Malformed` when the file has another form.
    pub fn decode(data: &[u8], kek: &[u8; 32]) -> Result<Self, Error> {
        if data.len() < MAGIC.len() + 1 || &data[..MAGIC.len()] != MAGIC { return Err(Error::Malformed); }
        let count = data[MAGIC.len()] as usize;
        if count > NETWORKS || data.len() != MAGIC.len() + 1 + count * RECORD { return Err(Error::Malformed); }
        let wrap = KekAes256::from(*kek);
        let mut kept = Kept::default();
        for (i, record) in data[MAGIC.len() + 1..].chunks_exact(RECORD).enumerate() {
            let mut plain = [0u8; PLAIN];
            let opened = wrap.unwrap(record, &mut plain).is_ok();
            let len = plain[0] as usize;
            let whole = opened && (1..=32).contains(&len) && plain[1 + len..40].iter().all(|&b| b == 0);
            let named = whole && !kept.names().any(|name| name == &plain[1..1 + len]);
            if named {
                let (mut name, mut pmk) = ([0u8; 32], [0u8; 32]);
                name.copy_from_slice(&plain[1..33]);
                pmk.copy_from_slice(&plain[40..]);
                kept.networks[i] = Some(Network { name, len: len as u8, pmk });
            }
            plain.fill(0);
            if !opened { return Err(Error::KeyData); }
            if !named { return Err(Error::Malformed); }
        }
        Ok(kept)
    }
}

impl Drop for Kept {
    fn drop(&mut self) { for network in self.networks.iter_mut().flatten() { network.pmk.fill(0); } }
}
