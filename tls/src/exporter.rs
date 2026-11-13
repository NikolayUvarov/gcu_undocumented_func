// The TLS 1.3 exporter (RFC 8446, section 7.5; tls.wit 1.4, 111-NET-0073). rustls's unbuffered connections do not
// offer it, so each connection's key log keeps the exporter secret alone, and the keying material is derived here.
extern crate alloc;
use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use hmac::digest::core_api::BlockSizeUser;
use hmac::{Mac, SimpleHmac};
use sha2::{Digest, Sha256, Sha384};

/// The most keying material one call derives.
pub const LENGTH_MAX: usize = 64;
const SECRET_MAX: usize = 48; // SHA-384's length

/// The hash of a TLS 1.3 suite's key schedule.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hash { Sha256, Sha384 }

fn wipe(bytes: &mut [u8]) {
    for byte in bytes.iter_mut() { unsafe { core::ptr::write_volatile(byte, 0) } }
}

fn hmac<D: Digest + BlockSizeUser + Clone>(key: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    let mut mac = <SimpleHmac<D> as Mac>::new_from_slice(key).expect("HMAC takes keys of any length");
    for part in parts { mac.update(part); }
    mac.finalize().into_bytes().to_vec()
}

// HKDF-Expand-Label (RFC 8446, section 7.1) into `out`.
fn expand_label<D: Digest + BlockSizeUser + Clone>(secret: &[u8], label: &[u8], context: &[u8], out: &mut [u8]) {
    let mut info = Vec::with_capacity(4 + 6 + label.len() + context.len());
    info.extend_from_slice(&(out.len() as u16).to_be_bytes());
    info.push((6 + label.len()) as u8);
    info.extend_from_slice(b"tls13 ");
    info.extend_from_slice(label);
    info.push(context.len() as u8);
    info.extend_from_slice(context);
    let (mut block, mut done, mut counter) = (Vec::new(), 0, 1u8);
    while done < out.len() {
        let next = hmac::<D>(secret, &[&block, &info, &[counter]]);
        wipe(&mut block);
        block = next;
        let n = block.len().min(out.len() - done);
        out[done..done + n].copy_from_slice(&block[..n]);
        (done, counter) = (done + n, counter + 1);
    }
    wipe(&mut block);
}

fn derive<D: Digest + BlockSizeUser + Clone>(secret: &[u8], label: &[u8], context: &[u8], out: &mut [u8]) {
    let mut derived = alloc::vec![0u8; <D as Digest>::output_size()];
    expand_label::<D>(secret, label, &D::digest(b""), &mut derived);
    expand_label::<D>(&derived, b"exporter", &D::digest(context), out);
    wipe(&mut derived);
}

/// TLS-Exporter(label, context, out.len()) from the connection's exporter secret. None for a label longer than 249
/// bytes or a length outside 1 to LENGTH_MAX.
pub fn export(hash: Hash, secret: &[u8], label: &[u8], context: &[u8], out: &mut [u8]) -> Option<()> {
    if label.len() > 249 || out.is_empty() || out.len() > LENGTH_MAX { return None; }
    match hash {
        Hash::Sha256 => derive::<Sha256>(secret, label, context, out),
        Hash::Sha384 => derive::<Sha384>(secret, label, context, out),
    }
    Some(())
}

/// One connection's exporter secret, which rustls logs once its handshake is done; nothing else it logs is kept.
pub struct Secret { bytes: [AtomicU8; SECRET_MAX], length: AtomicUsize }

impl Secret {
    pub fn new() -> Self { Self { bytes: [const { AtomicU8::new(0) }; SECRET_MAX], length: AtomicUsize::new(0) } }

    /// The secret, once logged.
    pub fn get(&self) -> Option<Vec<u8>> {
        let length = self.length.load(Ordering::Acquire);
        (length > 0).then(|| self.bytes[..length].iter().map(|b| b.load(Ordering::Relaxed)).collect())
    }

    /// Takes the secret of the key log's line `label`, when it is the exporter's.
    pub fn log(&self, label: &str, secret: &[u8]) {
        if label != "EXPORTER_SECRET" || secret.is_empty() || secret.len() > SECRET_MAX { return; }
        for (cell, &byte) in self.bytes.iter().zip(secret) { cell.store(byte, Ordering::Relaxed); }
        self.length.store(secret.len(), Ordering::Release);
    }
}

impl Default for Secret { fn default() -> Self { Self::new() } }

impl Drop for Secret {
    fn drop(&mut self) {
        for cell in self.bytes.iter() { cell.store(0, Ordering::Relaxed); }
        self.length.store(0, Ordering::Release);
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("Secret") }
}

/// Wipes a copy of the secret taken with `get`.
pub fn forget(mut copy: Vec<u8>) { wipe(&mut copy); }
