// The machine's layout (issue 205): where the GIC, its ITS, the console UART and the RTC are and which interrupts
// the UART and the virtual timer use. The defaults are QEMU `virt`'s; acpi::init replaces them with what the MADT
// (GICD, GICR, ITS), the SPCR (UART) and the GTDT (timer) say, so a board with UEFI and ACPI needs no code of its own
// here; GICv2 boards (the Raspberry Pi 4's GIC-400) as well as GICv3. The UART and the RTC start as none: on another
// board `virt`'s addresses may be RAM. The UART is the one the SPCR names; the PL031 is not in ACPI's static tables,
// so it is `virt`'s only on QEMU (the XSDT's OEM ID BOCHS).
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

pub static GICD: AtomicUsize = AtomicUsize::new(0x0800_0000);
pub static GICR: AtomicUsize = AtomicUsize::new(0x080A_0000); // the first redistributor
pub static GITS: AtomicUsize = AtomicUsize::new(0x0808_0000); // 0: no ITS
pub static UART: AtomicUsize = AtomicUsize::new(0); // a PL011 (or SBSA UART); 0: none
pub const VIRT_UART: usize = 0x0900_0000;
pub static UART_LINE: AtomicUsize = AtomicUsize::new(1); // SPI number - 32
pub static ENTRY_EL: AtomicUsize = AtomicUsize::new(1); // the EL the boot CPU entered the kernel at (100004-PRT-0014)
// The console's kind (from the SPCR always a PL011) and, for a 16550 a device tree names, where register n is (n << shift)
// and how wide it is read and written (100000-PRT-0010).
pub static UART_KIND: AtomicUsize = AtomicUsize::new(UART_PL011);
pub const UART_PL011: usize = 0;
pub const UART_16550: usize = 1;
pub static UART_SHIFT: AtomicUsize = AtomicUsize::new(0);
pub static UART_WIDTH: AtomicUsize = AtomicUsize::new(4);
pub static RTC: AtomicUsize = AtomicUsize::new(0); // a PL031; 0: none
pub const VIRT_RTC: usize = 0x0901_0000;
pub static RTC_LINE: AtomicUsize = AtomicUsize::new(2);
pub static TPM: AtomicUsize = AtomicUsize::new(0); // a TPM 2.0's locality 0 page (351-KRN-0052); 0: none
pub static WATCHDOG: AtomicUsize = AtomicUsize::new(0); // the Allwinner A31's watchdog (100004-PRT-0015); 0: none
pub static TIMER_PPI: AtomicUsize = AtomicUsize::new(27); // EL1 virtual timer
pub static GIC_VERSION: AtomicUsize = AtomicUsize::new(3); // 2: GICv2 (GIC-400), the CPU interface in memory
pub static GICC: AtomicUsize = AtomicUsize::new(0x0801_0000); // GICv2: the CPU interface (each CPU sees its own there)
pub static V2M: AtomicUsize = AtomicUsize::new(0); // GICv2m MSI frame; 0: none
// GICv2: each CPU's interface number (SGI targets), in cpu.rs's order of CPUs.
pub static INTERFACE: [AtomicUsize; crate::cpu::MAX] = [const { AtomicUsize::new(0) }; crate::cpu::MAX];
// The counter's erratum the device tree names for the A64: every read of the count retried as Linux does (clock.rs).
pub static COUNTER_SUN50I: AtomicBool = AtomicBool::new(false);
// Set for a device tree, which names no interface numbers: each CPU reads its own from the distributor (interrupts.rs).
pub static INTERFACE_FROM_GIC: AtomicBool = AtomicBool::new(false);

// RAM the device tree reserves (its reservation block, /reserved-memory), kept out of the frame pool even where the
// firmware's map calls it free (100000-PRT-0009): start and end, whole pages.
pub static RESERVED: [[AtomicU64; 2]; 16] = [const { [const { AtomicU64::new(0) }; 2] }; 16];
/// Records a reserved range; false when it is empty or the table is full.
pub fn reserve(start: u64, size: u64) -> bool {
    let (Some(end), true) = (start.checked_add(size), size != 0) else { return false };
    let (start, end) = (start & !0xFFF, end.checked_add(0xFFF).map_or(u64::MAX & !0xFFF, |e| e & !0xFFF));
    let Some(slot) = RESERVED.iter().find(|s| s[1].load(Ordering::Relaxed) == 0) else { return false };
    slot[0].store(start, Ordering::Relaxed);
    slot[1].store(end, Ordering::Relaxed);
    true
}

// Pin controllers (issue 206): base and size of each, by kind, as the DSDT and SSDTs name them; platform.rs lists them.
pub const PINS_MAX: usize = 4; // controllers of one kind
pub static PINS: [[AtomicUsize; 2]; 2 * PINS_MAX] = [const { [const { AtomicUsize::new(0) }; 2] }; 2 * PINS_MAX];
pub const PINS_BCM2711: usize = PINS_MAX; // PINS[..MAX] are PL061s, PINS[MAX..] BCM2711s

pub fn get(value: &AtomicUsize) -> usize { value.load(Ordering::Relaxed) }
pub fn set(value: &AtomicUsize, to: usize) { value.store(to, Ordering::Relaxed) }
