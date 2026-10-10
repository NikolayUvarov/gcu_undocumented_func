//! 351-ASR-0006: seeded fuzzing of the bootloader's manifest reader (bootloader/src/verify.rs; MC-9.1, 9.2, 12.2) with
//! the bootloader's own crates. Manifests are mutated and signed with a fixed test key: the reader must never panic,
//! must refuse a wrong signature and another format, and a file it confirms must have a line `file NAME SIZE SHA256`
//! that matches it. A run is evidence of its inputs, not a proof. MIND_FUZZ_SEED and MIND_FUZZ_ITERATIONS override the
//! fixed seed and count.
#[path = "../../../bootloader/src/verify.rs"]
#[allow(dead_code)]
mod verify;

#[cfg(test)]
mod tests {
    use super::verify::{Manifest, KEY};
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use std::panic::{self, AssertUnwindSafe};

    const SEED: u64 = 0x351A_0006;
    const ITERATIONS: usize = 20_000;
    const SIGNING_SEED: [u8; 32] = [0x35; 32];

    fn seed() -> u64 { std::env::var("MIND_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(SEED) }
    fn iterations() -> usize { std::env::var("MIND_FUZZ_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(ITERATIONS) }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
        fn below(&mut self, n: usize) -> usize { if n == 0 { 0 } else { (self.next() % n as u64) as usize } }
    }

    fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
        panic::catch_unwind(AssertUnwindSafe(f)).map_err(|e| e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())
    }

    fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }

    // The files of a release and its manifest, as scripts/sign_manifest.py writes one.
    fn release() -> (Vec<(&'static str, Vec<u8>)>, Vec<u8>) {
        let files = vec![("kernel.elf", b"\x7fELF kernel".to_vec()), ("shell.elf", b"\x7fELF shell".to_vec()), ("EFI/BOOT/BOOTX64.EFI", b"MZ".to_vec())];
        let mut text = String::from("MIND-MANIFEST 1\nkey test\n");
        for (name, data) in &files { text += &format!("file {} {} {}\n", name, data.len(), hex(&Sha256::digest(data))); }
        (files, text.into_bytes())
    }

    fn mutate(rng: &mut Rng, text: &[u8]) -> Vec<u8> {
        let mut lines: Vec<Vec<u8>> = text.split(|&b| b == b'\n').map(|l| l.to_vec()).collect();
        for _ in 0..1 + rng.below(3) {
            let i = rng.below(lines.len());
            match rng.below(9) {
                0 => { let l = lines[i].clone(); lines.insert(rng.below(lines.len() + 1), l); } // a line twice
                1 if lines.len() > 1 => { lines.remove(i); }
                2 => { let j = rng.below(lines.len()); lines.swap(i, j); }
                3 => { lines[i].extend_from_slice([b" extra".as_slice(), b" ", b"  0"][rng.below(3)]); }
                4 => { lines[i] = lines[i].iter().map(|c| c.to_ascii_uppercase()).collect(); }
                5 => { // a field changed: the size off by one, a sign, a non-digit
                    let line = String::from_utf8_lossy(&lines[i]).to_string();
                    let mut fields: Vec<String> = line.split(' ').map(String::from).collect();
                    if fields.len() > 2 { let k = 1 + rng.below(fields.len() - 1); fields[k] = [format!("+{}", fields[k]), format!("{}0", fields[k]), String::new(), "18446744073709551616".into(), "kernel.elf".into()][rng.below(5)].clone(); }
                    lines[i] = fields.join(" ").into_bytes();
                }
                6 if !lines[i].is_empty() => { let k = rng.below(lines[i].len()); lines[i][k] = rng.next() as u8; } // any byte, UTF-8 or not
                7 => { lines.insert(rng.below(lines.len() + 1), (0..rng.below(80)).map(|_| rng.next() as u8).collect()); }
                _ => { lines.insert(rng.below(lines.len() + 1), b"file kernel.elf 0 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_vec()); }
            }
        }
        lines.join(&b'\n')
    }

    #[test]
    fn the_key_built_in_is_the_signing_seeds() {
        assert_eq!(SigningKey::from_bytes(&SIGNING_SEED).verifying_key().to_bytes(), KEY);
    }

    #[test]
    fn signed_manifests_are_read_without_a_panic_and_consistently() {
        let key = SigningKey::from_bytes(&SIGNING_SEED);
        let (files, valid) = release();
        let mut rng = Rng(seed() | 1);
        let hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        let mut findings = std::collections::BTreeMap::<String, String>::new();
        let (mut verified, mut confirmed) = (0, 0);
        for _ in 0..iterations() {
            let text = if rng.below(10) == 0 { valid.clone() } else { mutate(&mut rng, &valid) };
            let good_signature = rng.below(20) != 0;
            let mut signature = key.sign(&text).to_bytes().to_vec();
            if !good_signature { let k = rng.below(64); signature[k] ^= 1 << rng.below(8); if rng.below(4) == 0 { signature.truncate(rng.below(64)); } }
            let manifest = match guarded(|| Manifest::verified(&text, &signature)) {
                Err(p) => { findings.entry(format!("verified panicked: {p}")).or_insert(String::from_utf8_lossy(&text).into()); continue; }
                Ok(Err(_)) => continue,
                Ok(Ok(m)) => m,
            };
            verified += 1;
            if !good_signature { findings.entry("accepted a wrong signature".into()).or_insert(String::from_utf8_lossy(&text).into()); }
            if !text.starts_with(b"MIND-MANIFEST 1\n") { findings.entry("accepted another format".into()).or_insert(String::from_utf8_lossy(&text).into()); }
            let _ = guarded(|| (manifest.key(), manifest.digest())).map_err(|p| findings.entry(format!("key or digest panicked: {p}")).or_insert(String::from_utf8_lossy(&text).into()));
            for (name, data) in files.iter().chain([("absent.elf", b"x".to_vec())].iter()) {
                let checked = match guarded(|| (manifest.check(name, data), manifest.names(name))) {
                    Ok(r) => r,
                    Err(p) => { findings.entry(format!("check or names panicked: {p}")).or_insert(String::from_utf8_lossy(&text).into()); continue; }
                };
                if checked.0.is_ok() {
                    confirmed += 1;
                    // Independently: a line whose fields are file, the name, the size and the hash.
                    let wanted = format!("{} {}", data.len(), hex(&Sha256::digest(data)));
                    let matches = text.split(|&b| b == b'\n').any(|l| { let s = String::from_utf8_lossy(l); let f: Vec<&str> = s.split(' ').collect(); f.len() >= 4 && f[0] == "file" && f[1] == *name && format!("{} {}", f[2].trim_start_matches('+').parse::<usize>().map(|n| n.to_string()).unwrap_or_default(), f[3].to_ascii_lowercase()) == wanted });
                    if !matches { findings.entry("confirmed a file no line of the manifest describes".into()).or_insert(format!("{name}: {}", String::from_utf8_lossy(&text))); }
                    if !checked.1 { findings.entry("confirmed a file it does not name".into()).or_insert(name.to_string()); }
                }
            }
        }
        panic::set_hook(hook);
        println!("manifests: {} inputs, {verified} verified, {confirmed} files confirmed (seed {:#x}), {} kinds of finding", iterations(), seed(), findings.len());
        for (kind, example) in &findings { println!("  {kind}\n    {:.200}", example); }
        assert!(findings.is_empty(), "manifest findings: {:?}", findings.keys().collect::<Vec<_>>());
    }
}
