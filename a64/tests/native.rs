//! The differential check (215-PRT-0016): the same Rust work computed natively and, built for
//! aarch64-unknown-none-softfloat as MIND Core's programs are, on the emulator, slot by slot. The aarch64 program is
//! built here by the toolchain's own rustc and linker, so the check needs nothing the build does not.
use a64::{run, Access, Bus, Cpu, Exit, Fault, FaultKind};
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

#[path = "programs/work.rs"]
mod work;

/// Memory from `base`, with no translation: the program is linked where it runs.
pub struct Flat { pub base: u64, pub memory: Vec<u8> }

impl Flat {
    fn at(&self, va: u64, size: u32, access: Access) -> Result<usize, Fault> {
        let offset = va.wrapping_sub(self.base);
        if offset.checked_add(size as u64).map_or(true, |end| end > self.memory.len() as u64) {
            return Err(Fault { address: va, access, kind: FaultKind::Translation(3) });
        }
        Ok(offset as usize)
    }
}

impl Bus for Flat {
    fn read(&mut self, va: u64, size: u32, access: Access) -> Result<u64, Fault> {
        let at = self.at(va, size, access)?;
        let mut bytes = [0u8; 8];
        bytes[..size as usize].copy_from_slice(&self.memory[at..at + size as usize]);
        Ok(u64::from_le_bytes(bytes))
    }
    fn write(&mut self, va: u64, size: u32, value: u64) -> Result<(), Fault> {
        let at = self.at(va, size, Access::Write)?;
        self.memory[at..at + size as usize].copy_from_slice(&value.to_le_bytes()[..size as usize]);
        Ok(())
    }
}

fn build_program() -> PathBuf {
    let programs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("a64-work.elf");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let status = Command::new(rustc)
        .args(["--edition=2021", "--target", "aarch64-unknown-none-softfloat", "-C", "opt-level=2", "-C", "panic=abort"])
        .args(["-C", "relocation-model=static", "--crate-type", "bin"])
        .arg(format!("-Clink-arg=-T{}", programs.join("link.ld").display()))
        .arg("-o").arg(&out)
        .arg(programs.join("aarch64_main.rs"))
        .status()
        .expect("rustc runs");
    assert!(status.success(), "the aarch64 program builds");
    out
}

/// Copies an ELF64 file's loaded segments into `flat` and returns its entry point.
pub fn load(elf: &[u8], flat: &mut Flat) -> u64 {
    let word = |at: usize| u64::from_le_bytes(elf[at..at + 8].try_into().unwrap());
    let half = |at: usize| u16::from_le_bytes(elf[at..at + 2].try_into().unwrap()) as usize;
    assert_eq!(&elf[..4], b"\x7fELF");
    assert_eq!(half(18), 183, "an aarch64 file");
    let (phoff, phentsize, phnum) = (word(32) as usize, half(54), half(56));
    for i in 0..phnum {
        let p = phoff + i * phentsize;
        if u32::from_le_bytes(elf[p..p + 4].try_into().unwrap()) != 1 { continue; }
        let (offset, vaddr, filesz, memsz) = (word(p + 8) as usize, word(p + 16), word(p + 32) as usize, word(p + 40) as usize);
        let at = (vaddr - flat.base) as usize;
        flat.memory[at..at + filesz].copy_from_slice(&elf[offset..offset + filesz]);
        flat.memory[at + filesz..at + memsz].fill(0);
    }
    word(24)
}

#[test]
fn the_same_work_agrees_natively_and_on_the_emulator() {
    let elf = std::fs::read(build_program()).unwrap();
    let mut flat = Flat { base: 0x10_0000, memory: vec![0; 4 << 20] };
    let entry = load(&elf, &mut flat);
    let mut cpu = Cpu::new();
    cpu.pc = entry;
    cpu.sp = flat.base + flat.memory.len() as u64 - 64;
    let start = Instant::now();
    let (exit, done) = run(&mut cpu, &mut flat, 4_000_000_000);
    let seconds = start.elapsed().as_secs_f64();
    assert_eq!(exit, Exit::Svc(0), "the program's end, at pc {:#x}", cpu.pc);
    assert_eq!(cpu.x[1] as usize, work::SLOTS);
    let emulated: Vec<u64> = (0..work::SLOTS as u64).map(|i| flat.read(cpu.x[0] + 8 * i, 8, Access::Read).unwrap()).collect();
    let mut native = [0u64; work::SLOTS];
    work::work(&mut native);
    eprintln!("{done} instructions in {seconds:.3} s: {:.0} million a second (this build's opt-level)", done as f64 / seconds / 1e6);
    let differ: Vec<usize> = (0..work::SLOTS).filter(|&i| emulated[i] != native[i]).collect();
    for &i in &differ { eprintln!("slot {i}: emulated {:#018x}, native {:#018x}", emulated[i], native[i]); }
    assert!(differ.is_empty(), "slots {differ:?} differ");
}
