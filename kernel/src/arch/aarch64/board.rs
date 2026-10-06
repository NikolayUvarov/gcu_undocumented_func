// The machine's layout (issue 205): where the GIC, its ITS, the console UART and the RTC are and which interrupts
// the UART and the virtual timer use. The defaults are QEMU `virt`'s; acpi::init replaces them with what the MADT
// (GICD, GICR, ITS), the SPCR (UART) and the GTDT (timer) say, so a board with UEFI and ACPI needs no code of its own
// here; GICv2 boards (the Raspberry Pi 4's GIC-400) as well as GICv3. The UART and the RTC start as none: on another
// board `virt`'s addresses may be RAM. The UART is the one the SPCR names; the PL031 is not in ACPI's static tables,
// so it is `virt`'s only on QEMU (the XSDT's OEM ID BOCHS).
use core::sync::atomic::{AtomicUsize, Ordering};

pub static GICD: AtomicUsize = AtomicUsize::new(0x0800_0000);
pub static GICR: AtomicUsize = AtomicUsize::new(0x080A_0000); // the first redistributor
pub static GITS: AtomicUsize = AtomicUsize::new(0x0808_0000); // 0: no ITS
pub static UART: AtomicUsize = AtomicUsize::new(0); // a PL011 (or SBSA UART); 0: none
pub const VIRT_UART: usize = 0x0900_0000;
pub static UART_LINE: AtomicUsize = AtomicUsize::new(1); // SPI number - 32
pub static RTC: AtomicUsize = AtomicUsize::new(0); // a PL031; 0: none
pub const VIRT_RTC: usize = 0x0901_0000;
pub static RTC_LINE: AtomicUsize = AtomicUsize::new(2);
pub static TIMER_PPI: AtomicUsize = AtomicUsize::new(27); // EL1 virtual timer
pub static GIC_VERSION: AtomicUsize = AtomicUsize::new(3); // 2: GICv2 (GIC-400), the CPU interface in memory
pub static GICC: AtomicUsize = AtomicUsize::new(0x0801_0000); // GICv2: the CPU interface (each CPU sees its own there)
pub static V2M: AtomicUsize = AtomicUsize::new(0); // GICv2m MSI frame; 0: none
// GICv2: each CPU's interface number (SGI targets), in cpu.rs's order of CPUs.
pub static INTERFACE: [AtomicUsize; crate::cpu::MAX] = [const { AtomicUsize::new(0) }; crate::cpu::MAX];

// Pin controllers (issue 206): base and size of each, by kind (PLATFORM_PINS_*), as the DSDT and SSDTs name them.
pub static PINS: [[AtomicUsize; 2]; 2 * crate::abi::PLATFORM_PINS_MAX] = [const { [const { AtomicUsize::new(0) }; 2] }; 2 * crate::abi::PLATFORM_PINS_MAX];
pub const PINS_BCM2711: usize = crate::abi::PLATFORM_PINS_MAX; // PINS[..MAX] are PL061s, PINS[MAX..] BCM2711s

pub fn get(value: &AtomicUsize) -> usize { value.load(Ordering::Relaxed) }
pub fn set(value: &AtomicUsize, to: usize) { value.store(to, Ordering::Relaxed) }
