// The keys built into the bootloader (351-UPD-0029): the root set's, from $MIND_ROOT_KEYS or the updater's
// keys/root-test.pub (`threshold T`, then a public key a line, as `release.py keys root` prints it), which sign the key
// files; the boot keys are the key file's. keys/test.pub is the test boot key's public half, only to name it.
use std::{env, fs, path::Path};

fn key(hex: &str) -> String {
    assert!(hex.len() == 64 && hex.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "a public key is 64 lower-case hex digits");
    format!("[{}]", (0..32).map(|i| format!("0x{}", &hex[2 * i..2 * i + 2])).collect::<Vec<_>>().join(", "))
}

fn main() {
    let test_path = "../updater/keys/root-test.pub";
    println!("cargo:rerun-if-env-changed=MIND_ROOT_KEYS");
    println!("cargo:rerun-if-changed={test_path}");
    println!("cargo:rerun-if-changed=keys/test.pub");
    let test = fs::read_to_string(test_path).expect("the root test set");
    let root = match env::var("MIND_ROOT_KEYS") {
        Ok(path) => { println!("cargo:rerun-if-changed={path}"); fs::read_to_string(&path).expect("MIND_ROOT_KEYS names a key file") }
        Err(_) => test.clone(),
    };
    let words: Vec<&str> = root.split_whitespace().collect();
    assert!(words.len() >= 3 && words[0] == "threshold", "MIND_ROOT_KEYS: `threshold T`, then a public key a line");
    let threshold: u8 = words[1].parse().expect("a threshold");
    let keys = &words[2..];
    assert!(keys.len() <= 4 && (1..=keys.len()).contains(&(threshold as usize)), "1 to 4 root keys and a threshold up to their number");
    assert!(keys.iter().enumerate().all(|(i, k)| !keys[..i].contains(k)), "the root keys are distinct");
    let boot_test = fs::read_to_string("keys/test.pub").expect("keys/test.pub");
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("boot_key.rs");
    fs::write(out, format!("pub const ROOT_KEYS: [[u8; 32]; {}] = [{}];\npub const ROOT_THRESHOLD: u8 = {};\npub const ROOT_TEST_KEYS: bool = {};\n\
                            pub const TEST_BOOT_KEY: [u8; 32] = {};\n",
        keys.len(), keys.iter().map(|k| key(k)).collect::<Vec<_>>().join(", "), threshold, root.trim() == test.trim(), key(boot_test.trim()))).unwrap();
}
