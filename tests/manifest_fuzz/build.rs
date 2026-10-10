// The key the reader checks with: the public half of the fixed test seed [0x35; 32] the fuzzer signs with.
use std::{env, fs, path::Path};

fn main() {
    let key = "a6d2455ea3a5771aba9fcb037924114c92f9f325049f6b4269e739d9048bb869";
    let bytes: Vec<String> = (0..32).map(|i| format!("0x{}", &key[2 * i..2 * i + 2])).collect();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("boot_key.rs");
    fs::write(out, format!("pub const KEY: [u8; 32] = [{}];\npub const TEST_KEY: bool = true;\n", bytes.join(", "))).unwrap();
}
