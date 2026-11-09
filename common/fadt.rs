#![allow(dead_code)]
// The FADT's fixed hardware (000-KRN-0216): its blocks of system I/O ports, the SCI line and how the power button is
// wired, read by the kernel (which ports it may lend) and by init (what it programs). Pure: the table's bytes in, no I/O.
// The table is firmware's, so untrusted: every field is bounds-checked and a block that runs past port 0xFFFF is none.

/// A block of system I/O ports the FADT names: its first port and its length in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block { pub port: u16, pub len: u16 }

impl Block {
    /// Whether ports `start..end` lie within the block.
    pub fn holds(&self, start: usize, end: usize) -> bool { start >= self.port as usize && end <= self.port as usize + self.len as usize && start < end }
}

/// What the FADT says about its fixed hardware (ACPI 6.5, 5.2.9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fixed {
    /// SCI_INT: the interrupt the SCI is wired to (an ISA line on a PC).
    pub sci: u16,
    /// SMI_CMD, one port, and ACPI_ENABLE, the byte written there to switch the chipset into ACPI mode.
    pub smi_cmd: Option<Block>,
    pub acpi_enable: u8,
    pub pm1a_evt: Option<Block>,
    pub pm1b_evt: Option<Block>,
    pub pm1a_cnt: Option<Block>,
    pub pm1b_cnt: Option<Block>,
    pub gpe0: Option<Block>,
    pub gpe1: Option<Block>,
    /// The fixed feature flags (offset 112).
    pub flags: u32,
}

/// PM1 status and enable bits of the fixed events (ACPI 6.5, 4.8.3.1): the power button's.
pub const PWRBTN: u16 = 1 << 8;
/// PM1 control: SCI_EN, set while the chipset is in ACPI mode.
pub const SCI_EN: u16 = 1;

impl Fixed {
    /// Flags bit 4 (PWR_BUTTON) clear: the power button is a fixed feature, read through PM1's status and enable.
    pub fn fixed_power_button(&self) -> bool { self.flags & 1 << 4 == 0 }
    /// Flags bit 20: a hardware-reduced platform, which has no fixed hardware at all.
    pub fn hardware_reduced(&self) -> bool { self.flags & 1 << 20 != 0 }
    /// Every block the FADT names in system I/O space: the ports the kernel may lend for them.
    pub fn blocks(&self) -> [Option<Block>; 7] { [self.smi_cmd, self.pm1a_evt, self.pm1b_evt, self.pm1a_cnt, self.pm1b_cnt, self.gpe0, self.gpe1] }
    /// A PM1 event block's status register (its first half) and its enable register (its second half).
    pub fn status_enable(block: Block) -> (u16, u16) { (block.port, block.port + block.len / 2) }
}

fn at<const N: usize>(t: &[u8], offset: usize) -> Option<[u8; N]> { t.get(offset..offset + N)?.try_into().ok() }
fn u32_at(t: &[u8], offset: usize) -> Option<u32> { at::<4>(t, offset).map(u32::from_le_bytes) }

// A block of `len` ports at `port`, if it is one: nonzero, and ending at port 0xFFFF at the latest.
fn block(port: u64, len: u64) -> Option<Block> {
    (port != 0 && len != 0 && port + len <= 0x1_0000).then(|| Block { port: port as u16, len: len as u16 })
}

// A block given by its 32-bit port (`legacy`), its length byte (`length`), and the generic address that supersedes it
// (`extended`, from revision 2): the generic address wins when nonzero; one outside system I/O space is no block here.
fn named(t: &[u8], legacy: usize, length: usize, extended: usize) -> Option<Block> {
    let len = *t.get(length)? as u64;
    if let Some(gas) = at::<12>(t, extended) {
        let address = u64::from_le_bytes(gas[4..12].try_into().ok()?);
        if address != 0 {
            let len = if len != 0 { len } else { gas[1] as u64 / 8 };
            return if gas[0] == 1 { block(address, len) } else { None };
        }
    }
    block(u32_at(t, legacy)? as u64, len)
}

/// The fixed hardware of `fadt`, the whole table with its header; None for another table or one too short to name it.
pub fn parse(fadt: &[u8]) -> Option<Fixed> {
    if fadt.get(..4)? != b"FACP" || fadt.len() < 116 { return None; }
    let smi = u32_at(fadt, 48)? as u64;
    let even = |b: Option<Block>, least: u16| b.filter(|b| b.len >= least && b.len % 2 == 0);
    Some(Fixed {
        sci: u16::from_le_bytes(at::<2>(fadt, 46)?),
        smi_cmd: block(smi, 1),
        acpi_enable: fadt[52],
        pm1a_evt: even(named(fadt, 56, 88, 148), 4),
        pm1b_evt: even(named(fadt, 60, 88, 160), 4),
        pm1a_cnt: named(fadt, 64, 89, 172).filter(|b| b.len >= 2),
        pm1b_cnt: named(fadt, 68, 89, 184).filter(|b| b.len >= 2),
        gpe0: even(named(fadt, 80, 92, 220), 2),
        gpe1: even(named(fadt, 84, 93, 232), 2),
        flags: u32_at(fadt, 112)?,
    })
}
