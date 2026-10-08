// The public key the bootloader checks boot manifests with (350-UPD-0003): the file named by $MIND_BOOT_PUBLIC_KEY
// (64 hex digits), or the public test key in keys/test.pub.
use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-env-changed=MIND_BOOT_PUBLIC_KEY");
    println!("cargo:rerun-if-changed=keys/test.pub");
    let test = fs::read_to_string("keys/test.pub").expect("keys/test.pub");
    let text = match env::var("MIND_BOOT_PUBLIC_KEY") {
        Ok(path) => { println!("cargo:rerun-if-changed={path}"); fs::read_to_string(&path).expect("MIND_BOOT_PUBLIC_KEY names a key file") }
        Err(_) => test.clone(),
    };
    let hex = text.trim();
    assert!(hex.len() == 64 && hex.bytes().all(|c| c.is_ascii_hexdigit()), "a public key is 64 hex digits");
    let bytes: Vec<String> = (0..32).map(|i| format!("0x{}", &hex[2 * i..2 * i + 2])).collect();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("boot_key.rs");
    fs::write(out, format!("pub const KEY: [u8; 32] = [{}];\npub const TEST_KEY: bool = {};\n", bytes.join(", "), hex == test.trim())).unwrap();
}
