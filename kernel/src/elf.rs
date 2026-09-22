// Checked static PIE loader. Each launch loads the original file again, so no
// writable globals, .bss, GOT pointers or application-local stacks are shared.
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;

fn u16_at(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(data[at..at + 2].try_into().unwrap())
}
fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(data[at..at + 4].try_into().unwrap())
}
fn word(data: &[u8], at: usize) -> usize {
    u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) as usize
}
fn range(start: usize, len: usize, limit: usize) -> Result<core::ops::Range<usize>, &'static str> {
    let end = start.checked_add(len).ok_or("ELF RANGE OVERFLOW")?;
    if end > limit {
        return Err("ELF RANGE OUT OF BOUNDS");
    }
    Ok(start..end)
}

pub struct Image<'a> {
    data: &'a [u8],
    headers: &'a [u8],
    min: usize,
    pub size: usize,
    entry: usize,
}

impl<'a> Image<'a> {
    pub fn segments(&self) -> impl Iterator<Item = (usize, usize, u32)> + '_ {
        self.headers
            .chunks_exact(56)
            .filter(|p| u32_at(p, 0) == PT_LOAD && word(p, 40) != 0)
            .map(|p| (word(p, 16) - self.min, word(p, 40), u32_at(p, 4)))
    }
    pub fn parse(data: &'a [u8]) -> Result<Self, &'static str> {
        if data.len() < 64
            || &data[..7] != b"\x7fELF\x02\x01\x01"
            || u16_at(data, 16) != 3
            || u16_at(data, 18) != 62
            || u32_at(data, 20) != 1
            || u16_at(data, 52) != 64
            || u16_at(data, 54) != 56
        {
            return Err("EXPECTED X86-64 STATIC PIE ELF");
        }
        let headers = &data[range(word(data, 32), u16_at(data, 56) as usize * 56, data.len())?];
        let mut min = usize::MAX;
        let mut max = 0;
        let entry = word(data, 24);
        let mut executable_entry = false;
        let mut dynamic_count = 0;
        for p in headers.chunks_exact(56) {
            match u32_at(p, 0) {
                PT_LOAD => {
                    let (offset, address, filesz, memsz, align) = (
                        word(p, 8),
                        word(p, 16),
                        word(p, 32),
                        word(p, 40),
                        word(p, 48),
                    );
                    if filesz > memsz
                        || (align > 1
                            && (!align.is_power_of_two()
                                || align > 4096
                                || address % align != offset % align))
                    {
                        return Err("INVALID ELF SEGMENT");
                    }
                    range(offset, filesz, data.len())?;
                    let end = address.checked_add(memsz).ok_or("ELF SEGMENT OVERFLOW")?;
                    if memsz != 0 {
                        min = min.min(address & !4095);
                        max = max.max(end);
                    }
                    executable_entry |= u32_at(p, 4) & 1 != 0 && entry >= address && entry < end;
                }
                PT_DYNAMIC => dynamic_count += 1,
                3 | 7 => return Err("ELF INTERPRETER/TLS NOT SUPPORTED"),
                _ => {}
            }
        }
        let size = max.checked_sub(min).ok_or("ELF HAS NO LOAD SEGMENTS")?;
        if size == 0 || size > 8 * 1024 * 1024 || !executable_entry || dynamic_count > 1 {
            return Err("INVALID ELF IMAGE OR ENTRY POINT");
        }
        Ok(Self {
            data,
            headers,
            min,
            size,
            entry,
        })
    }

    pub fn load(&self, memory: &mut [u8], base: usize) -> Result<usize, &'static str> {
        if memory.len() < self.size {
            return Err("ELF DESTINATION TOO SMALL");
        }
        memory.fill(0);
        for p in self
            .headers
            .chunks_exact(56)
            .filter(|p| u32_at(p, 0) == PT_LOAD)
        {
            let dest = word(p, 16)
                .checked_sub(self.min)
                .ok_or("ELF ADDRESS BELOW BASE")?;
            let filesz = word(p, 32);
            let to = range(dest, filesz, memory.len())?;
            memory[to].copy_from_slice(&self.data[range(word(p, 8), filesz, self.data.len())?]);
        }
        for p in self
            .headers
            .chunks_exact(56)
            .filter(|p| u32_at(p, 0) == PT_DYNAMIC)
        {
            crate::elf_reloc::apply(
                memory,
                self.min as u64,
                base as u64,
                word(p, 16) as u64,
                word(p, 32),
            )?;
        }
        base.checked_add(self.entry - self.min)
            .ok_or("ELF ENTRY OVERFLOW")
    }
}
