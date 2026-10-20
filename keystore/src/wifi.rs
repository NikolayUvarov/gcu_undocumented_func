// The WPA2-PSK networks the Wi-Fi station remembers (idl/keystore.wit 1.1, 550-NET-0024): their PMKs live only here,
// in `system/keystore/wifi.kept`, wrapped under a key derived from the device key (wpa::kept), so the file opens where
// the device key does: sealed by the TPM where there is one, else whoever has the disk reads both
// (docs/profile/threat-model.md). Only the station's badge reaches these (main.rs checks it); it gets each handshake's
// keys, never a PMK, at most BUDGET a minute, and every answer and refusal is logged.
use crate::stored::{self, wipe};
use mind::fs::{self, File};
use mind::idl::codec::List;
use mind::idl::keystore::{Error, WifiNetwork};
use wpa::kept::{self, Kept};

pub const FILE: &str = "system/keystore/wifi.kept";
// The station's country (1.3, 550-NET-0032): its two letters alone. A file cut short holds none, so the station sends nothing.
pub const COUNTRY: &str = "system/keystore/wifi.country";
const BUDGET: u32 = 60; // handshakes' keys a minute
const MINUTE_MS: usize = 60_000;
const LABEL: &[u8] = b"MIND keystore: remembered Wi-Fi networks\0";

pub struct Wifi { kept: Kept, kek: [u8; 32], minute: usize, used: u32 }

// An SSID as the log shows it: printable ASCII as it is, any other byte as a dot.
struct Shown<'a>(&'a [u8]);
impl core::fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for &b in self.0 { core::fmt::Write::write_char(f, if (0x20..0x7f).contains(&b) { b as char } else { '.' })?; }
        Ok(())
    }
}

impl Wifi {
    /// The networks kept under the device key whose seed is `seed`.
    pub fn load(seed: &[u8; 32]) -> Self {
        let mut material = [0u8; LABEL.len() + 32];
        material[..LABEL.len()].copy_from_slice(LABEL);
        material[LABEL.len()..].copy_from_slice(seed);
        let kek = mind::sha256::digest(&material);
        wipe(&mut material);
        let mut data = [0u8; kept::FILE_MAX + 1];
        let read = File::open(FILE).and_then(|file| file.read_at(0, &mut data));
        let kept = match read {
            Err(fs::Error::NotFound) => Kept::default(),
            Err(error) => { mind::println!("[KEYSTORE] REMEMBERED WI-FI NETWORKS UNREADABLE: {:?}", error); Kept::default() }
            Ok(n) => match Kept::decode(&data[..n], &kek) {
                Ok(kept) => { mind::println!("[KEYSTORE] WI-FI: {} REMEMBERED NETWORKS", kept.names().count()); kept }
                // Another device key (or a changed file): the networks are forgotten, and the file is replaced on the next change.
                Err(wpa::Error::KeyData) => { mind::println!("[KEYSTORE] REMEMBERED WI-FI NETWORKS DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN"); Kept::default() }
                Err(_) => { mind::println!("[KEYSTORE] REMEMBERED WI-FI NETWORKS DAMAGED: FORGOTTEN"); Kept::default() }
            },
        };
        wipe(&mut data);
        Wifi { kept, kek, minute: mind::time::uptime_ms(), used: 0 }
    }

    // Writes the networks kept, then flushes the volume.
    fn save(&self) -> Result<(), Error> {
        let mut data = [0u8; kept::FILE_MAX];
        let len = self.kept.encode(&self.kek, &mut data);
        let result = fs::mkdir(stored::DIR).and_then(|_| File::create(FILE)).and_then(|mut file| { file.write_at(0, &data[..len])?; file.flush() });
        wipe(&mut data);
        // The change holds in memory until the restart; the caller learns it was not kept (no-key).
        result.map_err(|error| { mind::println!("[KEYSTORE] REMEMBERED WI-FI NETWORKS NOT WRITTEN: {:?} (THIS BOOT ONLY)", error); Error::NoKey })
    }

    pub fn remember(&mut self, pid: u64, ssid: &[u8], pmk: &[u8]) -> Result<(), Error> {
        let pmk: &[u8; 32] = pmk.try_into().map_err(|_| Error::Invalid)?;
        self.kept.put(ssid, pmk).map_err(|e| if e == wpa::Error::Space { Error::Budget } else { Error::Invalid })?;
        self.save()?;
        mind::println!("[KEYSTORE] WI-FI NETWORK {} KEPT FOR PID {} ({} KEPT)", Shown(ssid), pid, self.kept.names().count());
        Ok(())
    }

    pub fn networks(&self, out: &mut List<WifiNetwork, 16>) -> Result<(), Error> {
        for name in self.kept.names() {
            let mut ssid = List::default();
            for &b in name { ssid.push(b); }
            out.push(WifiNetwork { ssid });
        }
        Ok(())
    }

    /// The handshake's KCK, KEK and TK.
    pub fn keys(&mut self, pid: u64, ssid: &[u8], ap: u64, own: u64, anonce: &[u8], snonce: &[u8]) -> Result<[u8; 48], Error> {
        let now = mind::time::uptime_ms();
        if now - self.minute >= MINUTE_MS { self.minute = now; self.used = 0; }
        let refuse = |why: &str, error: Error| { mind::println!("[KEYSTORE] REFUSED WI-FI KEYS FOR {} FOR PID {}: {}", Shown(ssid), pid, why); Err(error) };
        let (Ok(anonce), Ok(snonce)) = (<&[u8; 32]>::try_from(anonce), <&[u8; 32]>::try_from(snonce)) else { return refuse("NONCES NOT 32 BYTES", Error::Invalid) };
        let Some(pmk) = self.kept.get(ssid) else { return refuse("NOT KEPT", Error::Invalid) };
        if self.used >= BUDGET { return refuse("BUDGET USED", Error::Budget); }
        self.used += 1;
        let address = |a: u64| -> [u8; 6] { let b = a.to_be_bytes(); [b[2], b[3], b[4], b[5], b[6], b[7]] };
        let mut ptk = wpa::keys::ptk(pmk, &address(ap), &address(own), anonce, snonce);
        let mut keys = [0u8; 48];
        keys[..16].copy_from_slice(&ptk.kck);
        keys[16..32].copy_from_slice(&ptk.kek);
        keys[32..].copy_from_slice(&ptk.tk);
        for part in [&mut ptk.kck, &mut ptk.kek, &mut ptk.tk] { wipe(part); }
        mind::println!("[KEYSTORE] WI-FI KEYS FOR {} FOR PID {} ({} OF {} THIS MINUTE)", Shown(ssid), pid, self.used, BUDGET);
        Ok(keys)
    }

    pub fn forget(&mut self, pid: u64, ssid: &[u8]) -> Result<bool, Error> {
        if !self.kept.remove(ssid) { return Ok(false); }
        self.save()?;
        mind::println!("[KEYSTORE] WI-FI NETWORK {} FORGOTTEN FOR PID {}", Shown(ssid), pid);
        Ok(true)
    }
}

impl Drop for Wifi {
    fn drop(&mut self) { wipe(&mut self.kek); }
}

// Two capital letters: a country of the catalogue's form (`00`, the world domain, is never one).
fn country_code(code: &[u8]) -> Option<[u8; 2]> { <[u8; 2]>::try_from(code).ok().filter(|c| c.iter().all(u8::is_ascii_uppercase)) }

/// The station's country, if one is kept.
pub fn country() -> Option<[u8; 2]> {
    let mut data = [0u8; 3];
    let read = File::open(COUNTRY).and_then(|file| file.read_at(0, &mut data)).ok()?;
    country_code(&data[..read])
}

/// Keeps `code` as the station's country, replacing the one kept.
pub fn keep_country(pid: u64, code: &str) -> Result<(), Error> {
    let Some(code) = country_code(code.as_bytes()) else { mind::println!("[KEYSTORE] REFUSED WI-FI COUNTRY FOR PID {}: NOT TWO CAPITAL LETTERS", pid); return Err(Error::Invalid) };
    let shown = core::str::from_utf8(&code).unwrap_or("");
    let result = fs::mkdir(stored::DIR).and_then(|_| File::create(COUNTRY)).and_then(|mut file| { file.write_at(0, &code)?; file.flush() });
    match result {
        Ok(()) => { mind::println!("[KEYSTORE] WI-FI COUNTRY {} KEPT FOR PID {}", shown, pid); Ok(()) }
        Err(error) => { mind::println!("[KEYSTORE] WI-FI COUNTRY {} NOT WRITTEN: {:?}", shown, error); Err(Error::NoKey) }
    }
}
