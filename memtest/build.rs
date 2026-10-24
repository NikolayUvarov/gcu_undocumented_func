// Each program's linker script: memstack's asks for a 4 MiB stack in its PT_GNU_STACK header (262-KRN-0164).
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg-bin=memtest=-T{dir}/linker.ld");
    println!("cargo:rustc-link-arg-bin=memstack=-T{dir}/memstack.ld");
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rerun-if-changed=memstack.ld");
}
