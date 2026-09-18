// Static PIE relocation support: R_X86_64_RELATIVE stores load_bias + addend.
// https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf, section 4.4.1.
fn range(
    image_len: usize,
    min_vaddr: u64,
    address: u64,
    size: usize,
) -> Result<core::ops::Range<usize>, &'static str> {
    let start = usize::try_from(
        address
            .checked_sub(min_vaddr)
            .ok_or("ELF address below image")?,
    )
    .map_err(|_| "ELF address overflow")?;
    let end = start.checked_add(size).ok_or("ELF range overflow")?;
    if end > image_len {
        return Err("ELF range outside image");
    }
    Ok(start..end)
}

fn word(image: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(image[offset..offset + 8].try_into().unwrap())
}

pub fn apply(
    image: &mut [u8],
    min_vaddr: u64,
    load_addr: u64,
    dynamic_addr: u64,
    dynamic_size: usize,
) -> Result<(), &'static str> {
    let dynamic = range(image.len(), min_vaddr, dynamic_addr, dynamic_size)?;
    if dynamic_size % 16 != 0 {
        return Err("Invalid ELF dynamic table size");
    }
    let mut rela_addr = 0;
    let mut rela_size = 0;
    let mut rela_entry_size = 0;
    let mut terminated = false;
    for offset in (dynamic.start..dynamic.end).step_by(16) {
        let tag = word(image, offset);
        let value = word(image, offset + 8);
        match tag {
            0 => {
                terminated = true;
                break;
            }
            1 => return Err("Shared libraries are not supported"),
            7 => rela_addr = value,       // DT_RELA
            8 => rela_size = value,       // DT_RELASZ
            9 => rela_entry_size = value, // DT_RELAENT
            17 | 18 | 23 | 35 | 36 if value != 0 => return Err("Unsupported ELF relocation table"),
            _ => {}
        }
    }
    if !terminated {
        return Err("Unterminated ELF dynamic table");
    }
    if rela_size == 0 {
        return Ok(());
    }
    if rela_entry_size != 24 || rela_size % 24 != 0 {
        return Err("Invalid ELF RELA entry size");
    }
    let rela_size = usize::try_from(rela_size).map_err(|_| "ELF relocation size overflow")?;
    let relas = range(image.len(), min_vaddr, rela_addr, rela_size)?;
    let load_bias = load_addr.wrapping_sub(min_vaddr);
    for offset in (relas.start..relas.end).step_by(24) {
        let target = word(image, offset);
        let info = word(image, offset + 8);
        let addend = word(image, offset + 16);
        if info == 0 {
            continue;
        } // R_X86_64_NONE
        if info != 8 {
            return Err("Only symbol-free R_X86_64_RELATIVE is supported");
        }
        let dest = range(image.len(), min_vaddr, target, 8)?;
        if dest.start < relas.end && dest.end > relas.start {
            return Err("ELF relocation overwrites its own table");
        }
        image[dest].copy_from_slice(&load_bias.wrapping_add(addend).to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(image: &mut [u8], offset: usize, value: u64) {
        image[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn fixture(min: u64) -> [u8; 128] {
        let mut image = [0; 128];
        for (offset, tag, value) in [(0, 7, min + 64), (16, 8, 24), (32, 9, 24)] {
            put(&mut image, offset, tag);
            put(&mut image, offset + 8, value);
        }
        put(&mut image, 64, min + 96);
        put(&mut image, 72, 8);
        put(&mut image, 80, min + 120);
        image
    }

    #[test]
    fn rebases_function_pointers_for_zero_and_nonzero_link_addresses() {
        for min in [0, 0x2000] {
            let mut image = fixture(min);
            apply(&mut image, min, 0x100000, min, 64).unwrap();
            assert_eq!(word(&image, 96), 0x100078);
        }
    }

    #[test]
    fn accepts_signed_addends() {
        let mut image = fixture(0);
        put(&mut image, 80, (-16i64) as u64);
        apply(&mut image, 0, 0x100000, 0, 64).unwrap();
        assert_eq!(word(&image, 96), 0xFFFF0);
    }

    #[test]
    fn rejects_bad_targets_and_unsupported_relocations() {
        for (offset, value) in [
            (64, 125),
            (64, 64),
            (72, 1),
            (72, (1 << 32) | 8),
            (24, 25),
            (8, 120),
            (40, 16),
        ] {
            let mut image = fixture(0);
            put(&mut image, offset, value);
            assert!(apply(&mut image, 0, 0x100000, 0, 64).is_err());
        }
    }

    #[test]
    fn requires_bounded_terminated_dynamic_table() {
        let mut image = fixture(0);
        assert!(apply(&mut image, 0, 0x100000, 0, 48).is_err());
        assert!(apply(&mut image, 0, 0x100000, 120, 64).is_err());
        assert!(apply(&mut image, 0, 0x100000, 0, 63).is_err());
        assert!(apply(&mut [0; 16], 0, 0x100000, 0, 16).is_ok());
    }
}
