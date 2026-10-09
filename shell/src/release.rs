//! `release <file>` (351-APP-0019): a release channel file or a boot manifest, read by the parser service through the
//! shell's client (351-NET-0011) and shown only if the answer makes exactly the file's bytes, as the updater takes it.
//! It checks no signature: that is the updater's, with the release and boot keys.
use alloc::vec;
use core::fmt::Write;
use mind::abi::SLOT_PARSE;
use mind::ipc::Endpoint;
use mind::parse::{read_channel, read_manifest, Refused};
use mind::release::{FORMAT_LINE, MANIFEST_MAX};

const PARSE: Endpoint = Endpoint(SLOT_PARSE);

fn refused(out: &mut impl Write, why: Refused) {
    let _ = writeln!(out, "RELEASE: {}", match why {
        Refused::Service => "NO PARSER SERVICE",
        Refused::Malformed => "MALFORMED",
        Refused::Lied => "THE PARSER'S ANSWER DOES NOT MAKE THE FILE: NOT TAKEN",
    });
}

fn hex16(out: &mut impl Write, bytes: &[u8]) { for b in &bytes[..8] { let _ = write!(out, "{:02x}", b); } }

pub fn command(out: &mut impl Write, args: &[u8]) {
    let path = core::str::from_utf8(args).unwrap_or("").trim();
    if path.is_empty() || path.contains(' ') { let _ = writeln!(out, "RELEASE <CHANNEL FILE | MANIFEST>"); return; }
    let Ok(mut file) = mind::fs::File::open(path) else { let _ = writeln!(out, "RELEASE: CANNOT OPEN {}", path); return; };
    if file.size() > MANIFEST_MAX { let _ = writeln!(out, "RELEASE: LARGER THAN {} BYTES", MANIFEST_MAX); return; }
    let mut bytes = vec![0u8; file.size()];
    if file.read(&mut bytes) != Ok(bytes.len()) { let _ = writeln!(out, "RELEASE: CANNOT READ {}", path); return; }
    if bytes.starts_with(FORMAT_LINE.as_bytes()) {
        let mut boot = 0;
        match read_manifest(PARSE, &bytes, |file| if !file.path.as_str().contains('/') { boot += 1; }) {
            Ok((headers, files)) => { let _ = writeln!(out, "MANIFEST: {} HEADER LINES, {} FILES ({} AT THE ROOT); THE ANSWER MAKES THE TEXT", headers, files, boot); }
            Err(why) => refused(out, why),
        }
        return;
    }
    match read_channel(PARSE, &bytes) {
        Ok((channel, signed)) => {
            let _ = writeln!(out, "CHANNEL {}: VERSION {}, MINIMUM {}, EXPIRES {}", channel.name.as_str(), channel.version, channel.minimum, core::str::from_utf8(&channel.expires).unwrap_or("?"));
            for (arch, digest) in &channel.manifests[..channel.count] {
                let _ = write!(out, "  {} MANIFEST ", arch.as_str());
                hex16(out, digest);
                let _ = writeln!(out, "...");
            }
            let _ = writeln!(out, "SIGNED: THE FIRST {} BYTES; THE ANSWER MAKES THE FILE", signed.body);
        }
        Err(why) => refused(out, why),
    }
}
