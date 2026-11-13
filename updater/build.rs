// The public keys the updater checks releases with: the root set's (351-UPD-0026) from $MIND_ROOT_KEYS or
// keys/root-test.pub, `threshold T` and then a key a line, as `release.py keys root` prints it; the boot key's as the
// bootloader takes it ($MIND_BOOT_PUBLIC_KEY or its keys/test.pub). The release keys are the key file's.
use std::{env, fs};
use std::path::Path;

fn text(variable: &str, test: &str) -> (String, bool) {
    println!("cargo:rerun-if-env-changed={variable}");
    println!("cargo:rerun-if-changed={test}");
    let test = fs::read_to_string(test).expect("the public test file");
    let text = match env::var(variable) {
        Ok(path) => { println!("cargo:rerun-if-changed={path}"); fs::read_to_string(&path).unwrap_or_else(|_| panic!("{variable} names a key file")) }
        Err(_) => test.clone(),
    };
    let is_test = text.trim() == test.trim();
    (text, is_test)
}

fn key(hex: &str) -> String {
    assert!(hex.len() == 64 && hex.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "a public key is 64 lower-case hex digits");
    format!("[{}]", (0..32).map(|i| format!("0x{}", &hex[2 * i..2 * i + 2])).collect::<Vec<_>>().join(", "))
}

fn main() {
    let (root, root_test) = text("MIND_ROOT_KEYS", "keys/root-test.pub");
    let words: Vec<&str> = root.split_whitespace().collect();
    assert!(words.len() >= 3 && words[0] == "threshold", "MIND_ROOT_KEYS: `threshold T`, then a public key a line");
    let threshold: u8 = words[1].parse().expect("a threshold");
    let keys = &words[2..];
    assert!(keys.len() <= 4 && (1..=keys.len()).contains(&(threshold as usize)), "1 to 4 root keys and a threshold up to their number");
    assert!(keys.iter().enumerate().all(|(i, k)| !keys[..i].contains(k)), "the root keys are distinct");
    let (boot, boot_test) = text("MIND_BOOT_PUBLIC_KEY", "../bootloader/keys/test.pub");
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("keys.rs");
    fs::write(out, format!("pub const ROOT_KEYS: [[u8; 32]; {}] = [{}];\npub const ROOT_THRESHOLD: u8 = {};\npub const ROOT_TEST_KEYS: bool = {};\n\
                            pub const BOOT_KEY: [u8; 32] = {};\npub const BOOT_TEST_KEY: bool = {};\n",
        keys.len(), keys.iter().map(|k| key(k)).collect::<Vec<_>>().join(", "), threshold, root_test, key(boot.trim()), boot_test)).unwrap();
}
