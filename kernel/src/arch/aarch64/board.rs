// The machine's layout (issue 205): where the GIC, its ITS, the console UART and the RTC are and which interrupts
// the UART and the virtual timer use. The defaults are QEMU `virt`'s; acpi::init replaces them with what the MADT
// (GICD, GICR, ITS), the SPCR (UART) and the GTDT (timer) say, so a board with UEFI and ACPI needs no code of its own
// here. The PL031 is not in ACPI's static tables: its `virt` address stays unless a board says otherwise.
use core::sync::atomic::{AtomicUsize, Ordering};

pub static GICD: AtomicUsize = AtomicUsize::new(0x0800_0000);
pub static GICR: AtomicUsize = AtomicUsize::new(0x080A_0000); // the first redistributor
pub static GITS: AtomicUsize = AtomicUsize::new(0x0808_0000); // 0: no ITS
pub static UART: AtomicUsize = AtomicUsize::new(0x0900_0000); // a PL011 (or SBSA UART); 0: none
pub static UART_LINE: AtomicUsize = AtomicUsize::new(1); // SPI number - 32
pub static RTC: AtomicUsize = AtomicUsize::new(0x0901_0000); // a PL031; 0: none
pub static RTC_LINE: AtomicUsize = AtomicUsize::new(2);
pub static TIMER_PPI: AtomicUsize = AtomicUsize::new(27); // EL1 virtual timer

pub fn get(value: &AtomicUsize) -> usize { value.load(Ordering::Relaxed) }
pub fn set(value: &AtomicUsize, to: usize) { value.store(to, Ordering::Relaxed) }
