// The board from the device tree (100000-PRT-0009), when the firmware gives no ACPI (U-Boot's UEFI; Tow-Boot on the
// PinePhone): the GIC, the virtual timer's interrupt, the CPUs PSCI starts and its conduit, the console `stdout-path`
// names, a PL031, a generic ECAM and the reserved ranges, read by common/fdt.rs into board.rs's one record, as acpi.rs
// fills it from the MADT, GTDT, FADT, SPCR and MCFG. The tree is untrusted: what lies outside the identity map is not used.
use super::{acpi, board, fdt};
use core::fmt::Write;
use core::sync::atomic::Ordering;

const WINDOW: u64 = crate::mmu::IDENTITY_END;
// A device address the kernel can reach (its identity map), else 0: not used.
fn mapped(address: u64) -> usize { if address != 0 && address < WINDOW { address as usize } else { 0 } }

// The blob at `address`, its size from its header: at most 16 MiB, within the identity map.
unsafe fn blob(address: u64) -> Option<&'static [u8]> {
    if address == 0 || address.checked_add(40)? > WINDOW { return None; }
    let size = fdt::total_size(core::slice::from_raw_parts(address as *const u8, 40))?;
    if size < 40 || size > 1 << 24 || address.checked_add(size as u64)? > WINDOW { return None; }
    Some(core::slice::from_raw_parts(address as *const u8, size))
}

/// Fills board.rs from the device tree at `address`; false when there is none the kernel can read.
pub unsafe fn init(address: u64) -> bool {
    let Some(tree) = blob(address).and_then(fdt::Tree::open) else { return false };
    let b = fdt::board(&tree);
    // The console first, so that the lines below reach it.
    if let Some(uart) = b.console {
        let (kind, shift, width) = match uart.kind {
            fdt::UartKind::Pl011 => (board::UART_PL011, 0, 4),
            fdt::UartKind::Ns16550 { shift, width, .. } => (board::UART_16550, shift as usize, width as usize),
        };
        board::set(&board::UART_KIND, kind);
        board::set(&board::UART_SHIFT, shift);
        board::set(&board::UART_WIDTH, width);
        board::set(&board::UART, mapped(uart.base));
        if let Some(spi) = uart.spi { board::set(&board::UART_LINE, spi as usize); }
    }
    if let Some(gic) = b.gic {
        board::set(&board::GIC_VERSION, gic.version as usize);
        board::set(&board::GICD, mapped(gic.distributor));
        if gic.version == 2 {
            board::set(&board::GICC, mapped(gic.cpu_interface));
            board::set(&board::V2M, mapped(gic.v2m));
            board::set(&board::GITS, 0);
            // No interface numbers in the tree: each CPU reads its own from the distributor (interrupts.rs).
            board::INTERFACE_FROM_GIC.store(true, Ordering::Relaxed);
        } else {
            board::set(&board::GICR, mapped(gic.redistributor));
            board::set(&board::GITS, mapped(gic.its));
        }
    }
    if let Some(intid) = b.timer { board::set(&board::TIMER_PPI, intid as usize); }
    if let Some(conduit) = b.psci { acpi::set_conduit(conduit == fdt::Conduit::Smc); }
    if let Some((base, spi)) = b.rtc { board::set(&board::RTC, mapped(base)); if let Some(spi) = spi { board::set(&board::RTC_LINE, spi as usize); } }
    if let Some((base, first, last)) = b.ecam { crate::pcicfg::configure(base, first, last); }
    let mut count = 0;
    let (started, left) = fdt::cpus(&tree, |mpidr| {
        if count < acpi::CPUS.len() {
            acpi::CPUS[count].store(mpidr & 0xFF_00FF_FFFF, Ordering::Relaxed);
            acpi::INTERFACES[count].store(mpidr & 7, Ordering::Relaxed); // until the CPU reads its own
            count += 1;
        }
    });
    acpi::CPU_COUNT.store(count, Ordering::Release);
    let mut reserved = 0;
    fdt::reserved(&tree, |start, size| { if board::reserve(start, size) { reserved += 1; } });
    let mut out = crate::PanicSerial;
    let uart = match (board::get(&board::UART_KIND), board::get(&board::UART_SHIFT), board::get(&board::UART_WIDTH)) {
        (board::UART_16550, shift, width) => ("16550", shift, width), _ => ("PL011", 0, 4),
    };
    let _ = writeln!(out, "MIND CORE KERNEL: BOARD FROM THE DEVICE TREE: GICV{} GICD={:#x} GICC={:#x} GICR={:#x} ITS={:#x} V2M={:#x} UART={:#x} {} SHIFT {} WIDTH {} LINE {} TIMER PPI {} CPUS {} PSCI {} RTC={:#x} ECAM {} RESERVED {}\r",
        board::get(&board::GIC_VERSION), board::get(&board::GICD), board::get(&board::GICC), board::get(&board::GICR), board::get(&board::GITS), board::get(&board::V2M),
        board::get(&board::UART), uart.0, uart.1, uart.2, board::get(&board::UART_LINE), board::get(&board::TIMER_PPI), started,
        match b.psci { Some(fdt::Conduit::Smc) => "SMC", Some(fdt::Conduit::Hvc) => "HVC", None => "NONE" }, board::get(&board::RTC),
        if b.ecam.is_some() { "YES" } else { "NONE" }, reserved);
    if b.console.is_none() { crate::serial_print("MIND CORE KERNEL: DEVICE TREE: NO CONSOLE THIS KERNEL DRIVES IN stdout-path\n"); }
    if b.gic.is_none() { crate::serial_print("MIND CORE KERNEL: DEVICE TREE: NO GIC OF A KNOWN KIND\n"); }
    if left > 0 { let _ = writeln!(out, "MIND CORE KERNEL: DEVICE TREE: {} CPUS LEFT OUT (NOT STARTED THROUGH PSCI)\r", left); }
    if b.psci.is_none() { crate::serial_print("MIND CORE KERNEL: DEVICE TREE: NO PSCI 0.2: ONE CPU, NO RESTART\n"); }
    // The A64's counter can jump (its erratum): every read of it is retried as Linux does (100000-PRT-0012). Freescale's
    // erratum, whose reads are retried until two agree, is named only.
    match b.timer_erratum {
        Some(fdt::Erratum::Sun50iA64) => {
            board::COUNTER_SUN50I.store(true, Ordering::Relaxed);
            crate::serial_print("MIND CORE KERNEL: DEVICE TREE: THE TIMER NAMES THE A64'S COUNTER ERRATUM: EACH READ RETRIED WHILE ITS LOW BITS ARE ALL ONES OR ZEROS\n");
        }
        Some(fdt::Erratum::FslA008585) => crate::serial_print("MIND CORE KERNEL: DEVICE TREE: THE TIMER NAMES FREESCALE'S COUNTER ERRATUM A-008585, NOT WORKED AROUND\n"),
        None => {}
    }
    if let Some(f) = b.framebuffer { let _ = writeln!(out, "MIND CORE KERNEL: DEVICE TREE: A SIMPLE FRAMEBUFFER {}x{} AT {:#x}, NOT USED\r", f.width, f.height, f.base); }
    true
}
