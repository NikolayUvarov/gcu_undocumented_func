// The public keys the updater checks releases with (351-UPD-0007): the release key's from $MIND_RELEASE_PUBLIC_KEY or
// keys/release-test.pub, the boot key's as the bootloader takes it ($MIND_BOOT_PUBLIC_KEY or its keys/test.pub).
use std::{env, fs, path::Path};

// The key in a file of 64 hex digits, and whether it is the public test key of `test`.
fn key(variable: &str, test: &str) -> (String, bool) {
    println!("cargo:rerun-if-env-changed={variable}");
    println!("cargo:rerun-if-changed={test}");
    let test = fs::read_to_string(test).expect("the public test key");
    let text = match env::var(variable) {
        Ok(path) => { println!("cargo:rerun-if-changed={path}"); fs::read_to_string(&path).unwrap_or_else(|_| panic!("{variable} names a key file")) }
        Err(_) => test.clone(),
    };
    let hex = text.trim().to_string();
    assert!(hex.len() == 64 && hex.bytes().all(|c| c.is_ascii_hexdigit()), "a public key is 64 hex digits");
    let is_test = hex == test.trim();
    (hex, is_test)
}

fn bytes(hex: &str) -> String { (0..32).map(|i| format!("0x{}", &hex[2 * i..2 * i + 2])).collect::<Vec<_>>().join(", ") }

fn main() {
    let (release, release_test) = key("MIND_RELEASE_PUBLIC_KEY", "keys/release-test.pub");
    let (boot, boot_test) = key("MIND_BOOT_PUBLIC_KEY", "../bootloader/keys/test.pub");
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("keys.rs");
    fs::write(out, format!("pub const RELEASE_KEY: [u8; 32] = [{}];\npub const RELEASE_TEST_KEY: bool = {};\npub const BOOT_KEY: [u8; 32] = [{}];\npub const BOOT_TEST_KEY: bool = {};\n",
        bytes(&release), release_test, bytes(&boot), boot_test)).unwrap();
}
