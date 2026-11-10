//! Host tests of the device tree reader (common/fdt.rs, 100000-PRT-0009): QEMU `virt`'s trees (GICv2 as U-Boot hands
//! it to the loader, GICv3 as QEMU dumps it), the PinePhone's and the two Allwinner boards' (Linux v6.12, 100004-PRT-0013),
//! small trees made here for what those lack, and truncated, mutated and malformed blobs, which must yield nothing rather
//! than a panic.
#[path = "../common/fdt.rs"]
mod fdt;

use fdt::{board, console, cpus, reserved, Conduit, Tree, Uart, UartKind};

// QEMU 8.2's `virt,acpi=off,gic-version=2,highmem=off` with four Cortex-A53 and 2 GiB, as U-Boot 2025.10 (Ubuntu's
// u-boot-qemu) hands it to the loader through the UEFI configuration table.
const VIRT_V2: &[u8] = include_bytes!("data/fdt/virt-gicv2-uboot.dtb");
// QEMU 8.2's `virt,gic-version=3,highmem=off`, `-cpu max -smp 4`, from `-machine dumpdtb`, compacted by dtc.
const VIRT_V3: &[u8] = include_bytes!("data/fdt/virt-gicv3.dtb");
// Linux v6.12's sun50i-a64-pinephone-1.2.dts, compiled without sun50i-a64-cpu-opp.dtsi (THIRD_PARTY.md).
const PINEPHONE: &[u8] = include_bytes!("data/fdt/pinephone-1.2.dtb");
// Linux v6.12's sun50i-h5-orangepi-zero-plus2.dts and sun50i-h618-orangepi-zero3.dts (THIRD_PARTY.md, 100004-PRT-0013).
const ZERO_PLUS2: &[u8] = include_bytes!("data/fdt/orangepi-zero-plus2-h5.dtb");
const ZERO3: &[u8] = include_bytes!("data/fdt/orangepi-zero3-h618.dtb");

fn mpidrs(tree: &Tree) -> (Vec<u64>, usize) { let mut all = Vec::new(); let (_, left) = cpus(tree, |m| all.push(m)); (all, left) }

#[test]
fn qemus_gicv2_tree_through_u_boot() {
    let tree = Tree::open(VIRT_V2).unwrap();
    assert_eq!((tree.version, tree.size), (17, VIRT_V2.len()));
    let b = board(&tree);
    let gic = b.gic.unwrap();
    assert_eq!((gic.version, gic.distributor, gic.cpu_interface, gic.v2m, gic.redistributor, gic.its), (2, 0x0800_0000, 0x0801_0000, 0x0802_0000, 0, 0));
    assert_eq!(b.timer, Some(27));
    assert_eq!(b.timer_erratum, None);
    assert_eq!(b.psci, Some(Conduit::Hvc));
    assert_eq!(b.console, Some(Uart { base: 0x0900_0000, kind: UartKind::Pl011, spi: Some(1), speed: None }));
    assert_eq!(b.rtc, Some((0x0901_0000, Some(2))));
    assert_eq!(b.ecam, Some((0x3F00_0000, 0, 15)));
    assert_eq!(b.framebuffer, None);
    assert_eq!(mpidrs(&tree), (vec![0, 1, 2, 3], 0));
    assert_eq!(tree.at_path(b"/memory@40000000").unwrap().first_reg(), Some((0x4000_0000, 0x8000_0000)));
    assert_eq!(tree.at_path(b"/chosen").unwrap().string("u-boot,version"), Some(&b"2025.10-0ubuntu0.24.04.4"[..]));
}

#[test]
fn qemus_gicv3_tree() {
    let tree = Tree::open(VIRT_V3).unwrap();
    let b = board(&tree);
    let gic = b.gic.unwrap();
    assert_eq!((gic.version, gic.distributor, gic.redistributor, gic.its, gic.cpu_interface, gic.v2m), (3, 0x0800_0000, 0x080A_0000, 0x0808_0000, 0, 0));
    assert_eq!((b.timer, b.psci), (Some(27), Some(Conduit::Hvc)));
    assert_eq!(b.console.map(|u| (u.base, u.kind, u.spi)), Some((0x0900_0000, UartKind::Pl011, Some(1))));
    assert_eq!(mpidrs(&tree).0.len(), 4);
}

#[test]
fn the_pinephones_tree() {
    let tree = Tree::open(PINEPHONE).unwrap();
    let b = board(&tree);
    let gic = b.gic.unwrap();
    assert_eq!((gic.version, gic.distributor, gic.cpu_interface, gic.v2m), (2, 0x01C8_1000, 0x01C8_2000, 0));
    assert_eq!(b.timer, Some(27));
    assert_eq!(b.timer_erratum, Some(fdt::Erratum::Sun50iA64), "the A64's counter erratum is named");
    assert_eq!(b.psci, Some(Conduit::Smc));
    // UART0 through the alias serial0, a DesignWare 16550 with 32-bit registers, SPI 0, 115200 from stdout-path.
    assert_eq!(b.console, Some(Uart { base: 0x01C2_8000, kind: UartKind::Ns16550 { shift: 2, width: 4, designware: true }, spi: Some(0), speed: Some(115_200) }));
    assert_eq!(console(&tree), b.console);
    // No PL031, no PCI; the firmware's framebuffers are disabled until it fills one in.
    assert_eq!((b.rtc, b.ecam, b.framebuffer), (None, None, None));
    assert_eq!(mpidrs(&tree), (vec![0, 1, 2, 3], 0));
    let mut ranges = 0;
    reserved(&tree, |_, _| ranges += 1);
    assert_eq!(ranges, 0);
    assert_eq!(tree.resolve(b"serial0").unwrap().name(), b"serial@1c28000");
    assert_eq!(tree.at_path(b"/soc/serial@1c28000").unwrap().first_reg(), Some((0x01C2_8000, 0x400)));
    assert!(tree.at_path(b"/soc/serial").is_some(), "a part without a unit address matches");
}

// The status of the node at `path`: None without the node.
fn status<'a>(tree: &'a Tree, path: &[u8]) -> Option<&'a [u8]> { tree.at_path(path).map(|node| node.string("status").unwrap_or(b"okay")) }

#[test]
fn the_orange_pi_zero_plus2s_tree() {
    // Allwinner H5: the A64's addresses for the GIC-400 and UART0, four Cortex-A53, PSCI by SMC to TF-A.
    let tree = Tree::open(ZERO_PLUS2).unwrap();
    let b = board(&tree);
    let gic = b.gic.unwrap();
    assert_eq!((gic.version, gic.distributor, gic.cpu_interface, gic.v2m, gic.redistributor, gic.its), (2, 0x01C8_1000, 0x01C8_2000, 0, 0, 0));
    assert_eq!(b.timer, Some(27), "the virtual timer, PPI 11");
    assert_eq!(b.timer_erratum, None, "the A64's counter erratum is the A64's alone");
    assert_eq!(b.psci, Some(Conduit::Smc));
    // UART0 through serial0, a DesignWare 16550 with 32-bit registers 4 bytes apart, SPI 0, 115200 from stdout-path.
    assert_eq!(b.console, Some(Uart { base: 0x01C2_8000, kind: UartKind::Ns16550 { shift: 2, width: 4, designware: true }, spi: Some(0), speed: Some(115_200) }));
    assert_eq!(console(&tree), b.console);
    assert_eq!((b.rtc, b.ecam, b.framebuffer), (None, None, None));
    assert_eq!(mpidrs(&tree), (vec![0, 1, 2, 3], 0));
    // No memory node: U-Boot adds one at boot, and the loader takes memory from UEFI's map.
    assert!(tree.at_path(b"/memory").is_none());
    // UART1 (the AP6212's Bluetooth) is enabled in UART0's page: the console's page is not UART0's alone.
    assert_eq!(status(&tree, b"/soc/serial@1c28400"), Some(&b"okay"[..]));
    assert_eq!(status(&tree, b"/soc/serial@1c28800"), Some(&b"disabled"[..]));
    assert_eq!(status(&tree, b"/soc/serial@1c28c00"), Some(&b"disabled"[..]));
}

#[test]
fn the_orange_pi_zero3s_tree() {
    // Allwinner H618: the H6/H616 layout, the GIC-400 and UART0 elsewhere.
    let tree = Tree::open(ZERO3).unwrap();
    let b = board(&tree);
    let gic = b.gic.unwrap();
    assert_eq!((gic.version, gic.distributor, gic.cpu_interface, gic.v2m, gic.redistributor, gic.its), (2, 0x0302_1000, 0x0302_2000, 0, 0, 0));
    assert_eq!((b.timer, b.timer_erratum, b.psci), (Some(27), None, Some(Conduit::Smc)));
    assert_eq!(b.console, Some(Uart { base: 0x0500_0000, kind: UartKind::Ns16550 { shift: 2, width: 4, designware: true }, spi: Some(0), speed: Some(115_200) }));
    assert_eq!(console(&tree), b.console);
    assert_eq!((b.rtc, b.ecam, b.framebuffer), (None, None, None));
    assert_eq!(mpidrs(&tree), (vec![0, 1, 2, 3], 0));
    assert!(tree.at_path(b"/memory").is_none());
    // UART1 to UART5 share UART0's page, all disabled in this tree.
    for path in [&b"/soc/serial@5000400"[..], b"/soc/serial@5000800", b"/soc/serial@5000c00", b"/soc/serial@5001000", b"/soc/serial@5001400"] {
        assert_eq!(status(&tree, path), Some(&b"disabled"[..]), "{}", String::from_utf8_lossy(path));
    }
}

// A tree written here: the header, an empty reservation block unless given, the structure and the strings.
#[derive(Default)]
struct Writer { structure: Vec<u8>, strings: Vec<u8>, reservations: Vec<(u64, u64)> }
impl Writer {
    fn word(&mut self, w: u32) { self.structure.extend_from_slice(&w.to_be_bytes()); }
    fn pad(&mut self) { while self.structure.len() % 4 != 0 { self.structure.push(0); } }
    fn begin(&mut self, name: &str) -> &mut Self { self.word(1); self.structure.extend_from_slice(name.as_bytes()); self.structure.push(0); self.pad(); self }
    fn end(&mut self) -> &mut Self { self.word(2); self }
    fn prop(&mut self, name: &str, value: &[u8]) -> &mut Self {
        let offset = self.strings.len() as u32;
        self.strings.extend_from_slice(name.as_bytes());
        self.strings.push(0);
        self.word(3); self.word(value.len() as u32); self.word(offset);
        self.structure.extend_from_slice(value); self.pad(); self
    }
    fn cells(&mut self, name: &str, cells: &[u32]) -> &mut Self { let v: Vec<u8> = cells.iter().flat_map(|c| c.to_be_bytes()).collect(); self.prop(name, &v) }
    fn text(&mut self, name: &str, text: &str) -> &mut Self { let mut v = text.as_bytes().to_vec(); v.push(0); self.prop(name, &v) }
    fn finish(&mut self) -> Vec<u8> {
        self.word(9);
        let rsv_at = 40usize;
        let mut rsv: Vec<u8> = self.reservations.iter().flat_map(|&(a, s)| a.to_be_bytes().into_iter().chain(s.to_be_bytes())).collect();
        rsv.extend_from_slice(&[0; 16]);
        let struct_at = rsv_at + rsv.len();
        let strings_at = struct_at + self.structure.len();
        let total = strings_at + self.strings.len();
        let mut blob = Vec::new();
        for w in [0xd00d_feed, total as u32, struct_at as u32, strings_at as u32, rsv_at as u32, 17, 16, 0, self.strings.len() as u32, self.structure.len() as u32] { blob.extend_from_slice(&u32::to_be_bytes(w)); }
        blob.extend_from_slice(&rsv);
        blob.extend_from_slice(&self.structure);
        blob.extend_from_slice(&self.strings);
        blob
    }
}

// A board behind a bus that moves its addresses, a 16550 with byte registers chosen by an alias with options, and what
// a disabled node or a CPU PSCI does not start leaves out.
fn translated_board() -> Vec<u8> {
    let mut w = Writer { reservations: vec![(0x4800_0000, 0x10_0000)], ..Default::default() };
    w.begin("").cells("#address-cells", &[2]).cells("#size-cells", &[2]).cells("interrupt-parent", &[1]);
    w.begin("chosen").text("stdout-path", "uart1:9600n8").end();
    w.begin("aliases").text("uart1", "/bus@f0000000/serial@200").end();
    w.begin("psci").text("compatible", "arm,psci-0.2").text("method", "smc").end();
    w.begin("cpus").cells("#address-cells", &[2]).cells("#size-cells", &[0]);
    w.begin("cpu@0").text("device_type", "cpu").cells("reg", &[0, 0x0]).text("enable-method", "psci").end();
    w.begin("cpu@100").text("device_type", "cpu").cells("reg", &[0, 0x100]).text("enable-method", "psci").end();
    w.begin("cpu@200").text("device_type", "cpu").cells("reg", &[0, 0x200]).text("enable-method", "spin-table").end();
    w.begin("cpu@300").text("device_type", "cpu").cells("reg", &[0, 0x300]).text("enable-method", "psci").text("status", "disabled").end();
    w.end();
    // A bus at 0xF000_0000 whose children count from 0 in one cell each.
    w.begin("bus@f0000000").cells("#address-cells", &[1]).cells("#size-cells", &[1]).cells("ranges", &[0, 0, 0xF000_0000, 0x10_0000]);
    w.begin("interrupt-controller@1000").text("compatible", "arm,gic-400").prop("interrupt-controller", &[]).cells("#interrupt-cells", &[3]).cells("phandle", &[1])
        .cells("reg", &[0x1000, 0x1000, 0x2000, 0x2000]).end();
    w.begin("serial@200").text("compatible", "ns16550a").cells("reg", &[0x200, 0x100]).cells("interrupts", &[0, 5, 4]).end();
    w.begin("rtc@300").text("compatible", "arm,pl031").cells("reg", &[0x300, 0x100]).text("status", "disabled").end();
    w.end();
    // A bus without `ranges`: its children's addresses do not reach the CPU.
    w.begin("hidden").cells("#address-cells", &[1]).cells("#size-cells", &[1]);
    w.begin("timer").text("compatible", "arm,armv8-timer").cells("reg", &[0x10, 0x10]).cells("interrupts", &[1, 13, 4, 1, 14, 4, 1, 11, 4, 1, 10, 4]).end();
    w.end();
    w.begin("reserved-memory").cells("#address-cells", &[2]).cells("#size-cells", &[2]).prop("ranges", &[]);
    w.begin("tfa@40000000").cells("reg", &[0, 0x4000_0000, 0, 0x2_0000]).prop("no-map", &[]).end();
    w.end();
    w.end();
    w.finish()
}

#[test]
fn addresses_through_ranges_aliases_and_options() {
    let blob = translated_board();
    let tree = Tree::open(&blob).unwrap();
    let b = board(&tree);
    let gic = b.gic.unwrap();
    assert_eq!((gic.distributor, gic.cpu_interface), (0xF000_1000, 0xF000_2000));
    assert_eq!(b.console, Some(Uart { base: 0xF000_0200, kind: UartKind::Ns16550 { shift: 0, width: 1, designware: false }, spi: Some(5), speed: Some(9600) }));
    assert_eq!(b.psci, Some(Conduit::Smc));
    assert_eq!(b.rtc, None, "a disabled node is not used");
    // The timer's interrupts are still read (they need no address), but its reg cannot be translated.
    assert_eq!(b.timer, Some(27));
    assert_eq!(tree.at_path(b"/hidden/timer").unwrap().first_reg(), None);
    // Two CPUs PSCI starts; the spin table's is left out, the disabled one not counted.
    assert_eq!(mpidrs(&tree), (vec![0, 0x100], 1));
    let mut all = Vec::new();
    reserved(&tree, |a, s| all.push((a, s)));
    assert_eq!(all, vec![(0x4800_0000, 0x10_0000), (0x4000_0000, 0x2_0000)]);
}

#[test]
fn a_console_this_reader_does_not_drive_is_none() {
    for (compatible, shift, width) in [("snps,dw-apb-uart", 4, 4), ("snps,dw-apb-uart", 2, 3), ("samsung,exynos4210-uart", 0, 4)] {
        let mut w = Writer::default();
        w.begin("").cells("#address-cells", &[1]).cells("#size-cells", &[1]);
        w.begin("chosen").text("stdout-path", "/serial@100").end();
        w.begin("serial@100").text("compatible", compatible).cells("reg", &[0x100, 0x100]).cells("reg-shift", &[shift]).cells("reg-io-width", &[width]).end();
        w.end();
        assert_eq!(console(&Tree::open(&w.finish()).unwrap()), None, "{compatible} {shift} {width}");
    }
    // An alias to a missing node, and a stdout-path naming no alias.
    let mut w = Writer::default();
    w.begin("").begin("chosen").text("stdout-path", "serial9").end().begin("aliases").text("serial9", "/nowhere").end().end();
    assert_eq!(console(&Tree::open(&w.finish()).unwrap()), None);
}

#[test]
fn what_an_untrusted_blob_cannot_make_it_accept() {
    // Wrong magic, an older version, a size beyond the blob, a block beyond the size.
    let mut bad = VIRT_V2.to_vec();
    bad[0] = 0;
    assert!(Tree::open(&bad).is_none());
    let mut bad = VIRT_V2.to_vec();
    bad[20..24].copy_from_slice(&16u32.to_be_bytes());
    assert!(Tree::open(&bad).is_none());
    assert!(Tree::open(&VIRT_V2[..VIRT_V2.len() - 1]).is_none());
    let mut bad = VIRT_V2.to_vec();
    bad[36..40].copy_from_slice(&0x10_0000u32.to_be_bytes());
    assert!(Tree::open(&bad).is_none());
    // Every truncation of a valid tree, with its size made to fit: no panic, and never more nodes than the whole.
    let whole = { let t = Tree::open(PINEPHONE).unwrap(); let mut n = 0; assert!(t.walk(|_| { n += 1; true })); n };
    for len in (0..PINEPHONE.len()).step_by(7) {
        let mut cut = PINEPHONE[..len].to_vec();
        if cut.len() >= 8 { let l = len as u32; cut[4..8].copy_from_slice(&l.to_be_bytes()); }
        if let Some(t) = Tree::open(&cut) {
            let mut n = 0;
            t.walk(|_| { n += 1; true });
            assert!(n <= whole);
            let _ = board(&t);
        }
    }
    // Mutated bytes in the structure and strings, by a fixed generator: no panic.
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..1500 {
        let mut blob = PINEPHONE.to_vec();
        for _ in 0..4 {
            seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17;
            let at = 40 + (seed as usize) % (blob.len() - 40);
            blob[at] = (seed >> 32) as u8;
        }
        if let Some(t) = Tree::open(&blob) { let _ = board(&t); cpus(&t, |_| {}); reserved(&t, |_, _| {}); }
    }
}

#[test]
fn a_tree_too_deep_ends_the_walk_and_a_second_root_is_not_read() {
    let mut w = Writer::default();
    for i in 0..fdt::MAX_DEPTH + 1 { w.begin(&format!("n{i}")); }
    for _ in 0..fdt::MAX_DEPTH + 1 { w.end(); }
    let blob = w.finish();
    let tree = Tree::open(&blob).unwrap();
    let mut n = 0;
    assert!(!tree.walk(|_| { n += 1; true }));
    assert_eq!(n, fdt::MAX_DEPTH);
    // A second root is never reached: the walk ends with the first.
    let mut w = Writer::default();
    w.begin("").end().begin("second").end();
    let blob = w.finish();
    let mut names = Vec::new();
    assert!(Tree::open(&blob).unwrap().walk(|n| { names.push(n.name().to_vec()); true }));
    assert_eq!(names, vec![b"".to_vec()]);
    // A property outside any node.
    let mut w = Writer::default();
    w.prop("x", &[1]).begin("").end();
    assert!(!Tree::open(&w.finish()).unwrap().walk(|_| true));
}
