#![allow(dead_code)]
// A flattened device tree (the Devicetree Specification 0.4, chapter 5), read in place, and the board it describes
// (100000-PRT-0009): read by the aarch64 kernel when the firmware gives no ACPI (U-Boot, Tow-Boot on the PinePhone), by
// the bootloader for its console, and by the host tests. Pure: the blob's bytes in, no I/O and no allocation. The blob
// is the firmware's, so untrusted: every offset is bounds-checked, and a malformed tree yields nothing, never a panic.

pub const MAGIC: u32 = 0xd00d_feed;
/// Nodes deeper than this end the walk: the tree is not read below them.
pub const MAX_DEPTH: usize = 16;
const BEGIN_NODE: u32 = 1;
const END_NODE: u32 = 2;
const PROP: u32 = 3;
const NOP: u32 = 4;
const END: u32 = 9;
const HEADER: usize = 40;

fn be32(s: &[u8], at: usize) -> Option<u32> { Some(u32::from_be_bytes(s.get(at..at.checked_add(4)?)?.try_into().ok()?)) }
fn be64(s: &[u8], at: usize) -> Option<u64> { Some(u64::from_be_bytes(s.get(at..at.checked_add(8)?)?.try_into().ok()?)) }
fn align4(n: usize) -> Option<usize> { Some(n.checked_add(3)? & !3) }
fn cstr(s: &[u8], at: usize) -> Option<&[u8]> { let rest = s.get(at..)?; Some(&rest[..rest.iter().position(|&b| b == 0)?]) }
// A number of `cells` 32-bit cells (none, one or two) at `at`.
fn cells_at(s: &[u8], at: usize, cells: u32) -> Option<u64> {
    match cells { 0 => Some(0), 1 => be32(s, at).map(u64::from), 2 => be64(s, at), _ => None }
}

/// The blob's size from its header, for a caller that has mapped only the header so far.
pub fn total_size(header: &[u8]) -> Option<usize> { (be32(header, 0)? == MAGIC).then(|| be32(header, 4))?.map(|n| n as usize) }

#[derive(Clone, Copy)]
pub struct Tree<'a> {
    structure: &'a [u8],
    strings: &'a [u8],
    reservations: &'a [u8],
    /// The header's version (17) and the boot CPU's physical ID.
    pub version: u32,
    pub boot_cpu: u32,
    /// The blob's size, from its header.
    pub size: usize,
}

#[derive(Clone, Copy)]
enum Token<'a> { Begin(&'a [u8]), End, Prop(&'a [u8], &'a [u8]), Nop, Finish }

impl<'a> Tree<'a> {
    /// The tree in `blob`, which may run past it: a header of version 17 (or one compatible with it) whose blocks lie
    /// within its size and within `blob`. None for anything else.
    pub fn open(blob: &'a [u8]) -> Option<Tree<'a>> {
        if be32(blob, 0)? != MAGIC { return None; }
        let size = be32(blob, 4)? as usize;
        let (version, compatible) = (be32(blob, 20)?, be32(blob, 24)?);
        if size < HEADER || size > blob.len() || version < 17 || compatible > 17 { return None; }
        let blob = &blob[..size];
        let block = |offset: usize, len: usize| blob.get(be32(blob, offset)? as usize..)?.get(..be32(blob, len)? as usize);
        let structure = block(8, 36)?;
        let strings = block(12, 32)?;
        let reservations = blob.get(be32(blob, 16)? as usize..)?;
        if be32(blob, 8)? % 4 != 0 || be32(blob, 16)? % 8 != 0 { return None; }
        Some(Tree { structure, strings, reservations, version, boot_cpu: be32(blob, 28)?, size })
    }

    fn token(&self, at: usize) -> Option<(Token<'a>, usize)> {
        let s = self.structure;
        let body = at.checked_add(4)?;
        match be32(s, at)? {
            BEGIN_NODE => { let name = cstr(s, body)?; Some((Token::Begin(name), align4(body + name.len() + 1)?)) }
            END_NODE => Some((Token::End, body)),
            PROP => {
                let (len, name) = (be32(s, body)? as usize, be32(s, body + 4)? as usize);
                let start = body.checked_add(8)?;
                let value = s.get(start..start.checked_add(len)?)?;
                Some((Token::Prop(cstr(self.strings, name)?, value), align4(start + len)?))
            }
            NOP => Some((Token::Nop, body)),
            END => Some((Token::Finish, body)),
            _ => None,
        }
    }

    /// Calls `each` with every node, the root first, in the blob's order, until it answers false. False when the
    /// structure block is malformed (or deeper than MAX_DEPTH) before the walk ended; the nodes before it were seen.
    pub fn walk(&self, mut each: impl FnMut(&Node<'a>) -> bool) -> bool {
        let (mut at, mut depth, mut up, mut started) = (0usize, 0usize, [0usize; MAX_DEPTH], false);
        loop {
            let Some((token, next)) = self.token(at) else { return false };
            match token {
                Token::Begin(_) => {
                    if (started && depth == 0) || depth >= MAX_DEPTH { return false; }
                    let node = Node { tree: *self, at, depth, up };
                    up[depth] = at;
                    depth += 1;
                    started = true;
                    if !each(&node) { return true; }
                }
                Token::End => { if depth == 0 { return false; } depth -= 1; if depth == 0 { return true; } }
                Token::Prop(..) => { if depth == 0 { return false; } }
                Token::Nop => {}
                Token::Finish => return false,
            }
            at = next;
        }
    }

    /// The first node `test` accepts.
    pub fn find(&self, mut test: impl FnMut(&Node<'a>) -> bool) -> Option<Node<'a>> {
        let mut found = None;
        self.walk(|node| { if test(node) { found = Some(*node); false } else { true } });
        found
    }

    /// The node at `path` ("/soc/serial@1c28000"); a part without a unit address matches a name that has one.
    pub fn at_path(&self, path: &[u8]) -> Option<Node<'a>> {
        if path.first() != Some(&b'/') { return None; }
        let parts = || path.split(|&b| b == b'/').filter(|p| !p.is_empty());
        let count = parts().count();
        self.find(|node| node.depth == count && parts().enumerate().all(|(i, part)| node.ancestor(i + 1).is_some_and(|a| a.named(part))))
    }

    /// The node whose `phandle` is `handle`.
    pub fn by_phandle(&self, handle: u32) -> Option<Node<'a>> {
        self.find(|node| node.u32("phandle").or_else(|| node.u32("linux,phandle")) == Some(handle))
    }

    /// The node an alias of `/aliases` names, or the path itself when it starts with `/`.
    pub fn resolve(&self, name: &[u8]) -> Option<Node<'a>> {
        if name.first() == Some(&b'/') { return self.at_path(name); }
        let path = self.at_path(b"/aliases")?.props().find(|(n, _)| *n == name).map(|(_, v)| v)?;
        let path = &path[..path.iter().position(|&b| b == 0)?];
        (path.first() == Some(&b'/')).then(|| self.at_path(path))?
    }

    /// The console `/chosen` names in `stdout-path`, and the options after its `:` ("115200n8").
    pub fn stdout(&self) -> Option<(Node<'a>, &'a [u8])> {
        let chosen = self.at_path(b"/chosen")?;
        let path = chosen.string("stdout-path").or_else(|| chosen.string("linux,stdout-path"))?;
        let (name, options) = match path.iter().position(|&b| b == b':') { Some(i) => (&path[..i], &path[i + 1..]), None => (path, &path[path.len()..]) };
        Some((self.resolve(name)?, options))
    }

    /// The memory reservation block's entries (address, size), at most 64.
    pub fn reservations(&self) -> impl Iterator<Item = (u64, u64)> + 'a {
        let block = self.reservations;
        (0..64).map_while(move |i| { let (a, s) = (be64(block, i * 16)?, be64(block, i * 16 + 8)?); (a != 0 || s != 0).then_some((a, s)) })
    }
}

/// A node of the tree: where its token is, its depth, and its ancestors' tokens (the root's first).
#[derive(Clone, Copy)]
pub struct Node<'a> { tree: Tree<'a>, at: usize, depth: usize, up: [usize; MAX_DEPTH] }

pub struct Props<'a> { tree: Tree<'a>, at: usize }
impl<'a> Iterator for Props<'a> {
    type Item = (&'a [u8], &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (token, next) = self.tree.token(self.at)?;
            self.at = next;
            match token { Token::Prop(name, value) => return Some((name, value)), Token::Nop => {} _ => { self.at = usize::MAX; return None; } }
        }
    }
}

/// The addresses and sizes of a `reg`, translated to the CPU's addresses through the ancestors' `ranges`.
pub struct Regs<'a> { node: Node<'a>, value: &'a [u8], address: u32, size: u32, at: usize }
impl<'a> Iterator for Regs<'a> {
    type Item = (u64, u64);
    fn next(&mut self) -> Option<(u64, u64)> {
        let step = 4 * (self.address + self.size) as usize;
        if step == 0 || self.at + step > self.value.len() { return None; }
        let address = cells_at(self.value, self.at, self.address)?;
        let size = cells_at(self.value, self.at + 4 * self.address as usize, self.size)?;
        self.at += step;
        Some((self.node.translate(address)?, size))
    }
}

impl<'a> Node<'a> {
    fn at_token(tree: Tree<'a>, at: usize, depth: usize, up: [usize; MAX_DEPTH]) -> Node<'a> { Node { tree, at, depth, up } }
    /// Its name with the unit address ("serial@1c28000"); the root's is empty.
    pub fn name(&self) -> &'a [u8] { match self.tree.token(self.at) { Some((Token::Begin(name), _)) => name, _ => b"" } }
    fn named(&self, part: &[u8]) -> bool {
        let name = self.name();
        name == part || (!part.contains(&b'@') && name.split(|&b| b == b'@').next() == Some(part))
    }
    pub fn depth(&self) -> usize { self.depth }
    /// Its ancestor at `depth` (0 the root), or itself at its own depth.
    pub fn ancestor(&self, depth: usize) -> Option<Node<'a>> {
        if depth == self.depth { return Some(*self); }
        (depth < self.depth).then(|| Node::at_token(self.tree, self.up[depth], depth, self.up))
    }
    pub fn parent(&self) -> Option<Node<'a>> { self.depth.checked_sub(1).and_then(|d| self.ancestor(d)) }
    pub fn is_child_of(&self, other: &Node<'a>) -> bool { self.depth == other.depth + 1 && self.up[other.depth] == other.at }

    pub fn props(&self) -> Props<'a> {
        let at = match self.tree.token(self.at) { Some((Token::Begin(_), next)) => next, _ => usize::MAX };
        Props { tree: self.tree, at }
    }
    pub fn prop(&self, name: &str) -> Option<&'a [u8]> { self.props().find(|(n, _)| *n == name.as_bytes()).map(|(_, v)| v) }
    pub fn has(&self, name: &str) -> bool { self.prop(name).is_some() }
    pub fn u32(&self, name: &str) -> Option<u32> { self.prop(name).filter(|v| v.len() == 4).and_then(|v| be32(v, 0)) }
    /// A string property, without its NUL; None when it has none.
    pub fn string(&self, name: &str) -> Option<&'a [u8]> { let v = self.prop(name)?; Some(&v[..v.iter().position(|&b| b == 0)?]) }
    /// Whether one of its `compatible` strings is in `any`.
    pub fn compatible(&self, any: &[&str]) -> bool {
        self.prop("compatible").is_some_and(|v| v.split(|&b| b == 0).any(|c| any.iter().any(|a| a.as_bytes() == c)))
    }
    /// `status` absent, "okay" or "ok".
    pub fn enabled(&self) -> bool { self.string("status").map_or(true, |s| s == b"okay" || s == b"ok") }

    /// Its own `#address-cells` and `#size-cells` (2 and 1 when absent): how its children's `reg` reads.
    pub fn cells(&self) -> (u32, u32) { (self.u32("#address-cells").unwrap_or(2), self.u32("#size-cells").unwrap_or(1)) }

    /// Its `reg`, in its parent's cells; None without one, or with cells this reader does not take (more than two).
    pub fn reg(&self) -> Option<Regs<'a>> {
        let (address, size) = self.parent()?.cells();
        if address == 0 || address > 2 || size > 2 { return None; }
        Some(Regs { node: *self, value: self.prop("reg")?, address, size, at: 0 })
    }
    /// The first address and size of its `reg`.
    pub fn first_reg(&self) -> Option<(u64, u64)> { self.reg()?.next() }

    // `address`, in its parent's space, as the CPU sees it: through each ancestor's `ranges` up to the root. An ancestor
    // without `ranges` maps nothing (None); an empty one maps one to one.
    fn translate(&self, mut address: u64) -> Option<u64> {
        for depth in (1..self.depth).rev() {
            let bus = self.ancestor(depth)?;
            let ranges = bus.prop("ranges")?;
            if ranges.is_empty() { continue; }
            let ((child, size), parent) = (bus.cells(), self.ancestor(depth - 1)?.cells().0);
            if child == 0 || child > 2 || parent == 0 || parent > 2 || size == 0 || size > 2 { return None; }
            let entry = 4 * (child + parent + size) as usize;
            let mut found = None;
            for at in (0..ranges.len() / entry).map(|i| i * entry) {
                let (from, to, len) = (cells_at(ranges, at, child)?, cells_at(ranges, at + 4 * child as usize, parent)?, cells_at(ranges, at + 4 * (child + parent) as usize, size)?);
                if address >= from && address - from < len { found = Some(to.checked_add(address - from)?); break; }
            }
            address = found?;
        }
        Some(address)
    }

    /// The `interrupt-parent` it has or inherits.
    pub fn interrupt_parent(&self) -> Option<u32> { (0..=self.depth).rev().find_map(|d| self.ancestor(d)?.u32("interrupt-parent")) }
}

// The board.

/// How the console's registers are reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UartKind {
    /// Arm's PL011, or the SBSA generic UART (its subset).
    Pl011,
    /// A 16550: register `n` at `n << shift`, accessed `width` bytes wide; `designware` for Synopsys's DW APB UART
    /// (the A64's), whose busy-detect interrupt asks for its status register to be read.
    Ns16550 { shift: u8, width: u8, designware: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uart {
    pub base: u64,
    pub kind: UartKind,
    /// Its SPI (the interrupt line less 32), when its interrupt goes to the GIC.
    pub spi: Option<u32>,
    /// The speed in `stdout-path`'s options, else `current-speed`.
    pub speed: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gic {
    /// 2 (GIC-400 and its kind: the CPU interface in memory) or 3.
    pub version: u8,
    pub distributor: u64,
    /// GICv2: the CPU interface; 0 on GICv3.
    pub cpu_interface: u64,
    /// GICv3: the first redistributor; 0 on GICv2.
    pub redistributor: u64,
    /// GICv3's ITS, GICv2's v2m MSI frame; 0 when there is none.
    pub its: u64,
    pub v2m: u64,
    phandle: Option<u32>,
}

/// A counter erratum a timer node names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Erratum {
    /// `allwinner,erratum-unknown1`, the A64's: a read can be wrong, its low bits all ones or all zeros.
    Sun50iA64,
    /// `fsl,erratum-a008585`, Freescale's: a read can be wrong, until two reads agree.
    FslA008585,
}

/// PSCI's conduit, from `/psci`'s `method`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conduit { Smc, Hvc }

/// A framebuffer the firmware left on (`simple-framebuffer`): 32-bit pixels only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Framebuffer { pub base: u64, pub size: u64, pub width: u32, pub height: u32, pub stride: u32, pub bgr: bool }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Board {
    pub gic: Option<Gic>,
    /// The EL1 virtual timer's interrupt ID (a PPI: 16 + its number), from `arm,armv8-timer`'s third interrupt.
    pub timer: Option<u32>,
    /// A known erratum of the counter its timer node names.
    pub timer_erratum: Option<Erratum>,
    pub psci: Option<Conduit>,
    pub console: Option<Uart>,
    /// A PL031 real-time clock and its SPI.
    pub rtc: Option<(u64, Option<u32>)>,
    /// A generic ECAM PCI host: its base and its first and last bus.
    pub ecam: Option<(u64, u8, u8)>,
    pub framebuffer: Option<Framebuffer>,
}

const GIC_V2: [&str; 4] = ["arm,gic-400", "arm,cortex-a15-gic", "arm,cortex-a7-gic", "arm,cortex-a9-gic"];

fn enabled_compatible<'a>(tree: &Tree<'a>, any: &[&str]) -> Option<Node<'a>> { tree.find(|n| n.enabled() && n.compatible(any)) }

/// The interrupt controller: the first enabled GIC of a kind this reader knows.
pub fn gic(tree: &Tree) -> Option<Gic> {
    let node = tree.find(|n| n.enabled() && n.has("interrupt-controller") && (n.compatible(&["arm,gic-v3"]) || n.compatible(&GIC_V2)))?;
    let mut reg = node.reg()?;
    let phandle = node.u32("phandle").or_else(|| node.u32("linux,phandle"));
    let child = |kind: &str| tree.find(|n| n.is_child_of(&node) && n.enabled() && n.compatible(&[kind])).and_then(|n| n.first_reg()).map_or(0, |(a, _)| a);
    if node.compatible(&["arm,gic-v3"]) {
        let (distributor, redistributor) = (reg.next()?.0, reg.next()?.0);
        Some(Gic { version: 3, distributor, cpu_interface: 0, redistributor, its: child("arm,gic-v3-its"), v2m: 0, phandle })
    } else {
        let (distributor, cpu_interface) = (reg.next()?.0, reg.next()?.0);
        Some(Gic { version: 2, distributor, cpu_interface, redistributor: 0, its: 0, v2m: child("arm,gic-v2m-frame"), phandle })
    }
}

// Interrupt `index` of `node` when it goes to `gic` (three cells: kind, number, flags): (kind, number).
fn interrupt(node: &Node, gic: &Gic, index: usize) -> Option<(u32, u32)> {
    if gic.phandle.is_none() || node.interrupt_parent() != gic.phandle { return None; }
    let value = node.prop("interrupts")?;
    Some((be32(value, index * 12)?, be32(value, index * 12 + 4)?))
}
fn spi(node: &Node, gic: Option<&Gic>) -> Option<u32> { interrupt(node, gic?, 0).filter(|&(kind, _)| kind == 0).map(|(_, n)| n) }

fn decimal(text: &[u8]) -> Option<u32> {
    let digits = text.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || digits > 9 { return None; }
    text[..digits].iter().try_fold(0u32, |n, &d| n.checked_mul(10)?.checked_add((d - b'0') as u32))
}

/// The console `stdout-path` names, if it is a UART this reader knows, enabled, with an address.
pub fn console(tree: &Tree) -> Option<Uart> { console_with(tree, gic(tree).as_ref()) }

fn console_with(tree: &Tree, gic: Option<&Gic>) -> Option<Uart> {
    let (node, options) = tree.stdout()?;
    if !node.enabled() { return None; }
    let kind = if node.compatible(&["arm,pl011", "arm,sbsa-uart"]) { UartKind::Pl011 } else if node.compatible(&["snps,dw-apb-uart", "ns16550a", "ns16550", "ns16450"]) {
        let shift = node.u32("reg-shift").unwrap_or(0);
        let width = node.u32("reg-io-width").unwrap_or(1);
        if shift > 3 || !matches!(width, 1 | 2 | 4) { return None; }
        UartKind::Ns16550 { shift: shift as u8, width: width as u8, designware: node.compatible(&["snps,dw-apb-uart"]) }
    } else { return None };
    let (base, _) = node.first_reg()?;
    if base == 0 { return None; }
    Some(Uart { base, kind, spi: spi(&node, gic), speed: decimal(options).or_else(|| node.u32("current-speed")) })
}

/// What every board has, from the tree.
pub fn board(tree: &Tree) -> Board {
    let gic = gic(tree);
    let timer_node = enabled_compatible(tree, &["arm,armv8-timer"]);
    // The third of the timer's four interrupts is the EL1 virtual timer's, a PPI (kind 1).
    let timer = timer_node.zip(gic.as_ref()).and_then(|(n, g)| interrupt(&n, g, 2)).filter(|&(kind, n)| kind == 1 && n < 16).map(|(_, n)| 16 + n);
    let timer_erratum = timer_node.and_then(|n| if n.has("allwinner,erratum-unknown1") { Some(Erratum::Sun50iA64) } else if n.has("fsl,erratum-a008585") { Some(Erratum::FslA008585) } else { None });
    let psci = enabled_compatible(tree, &["arm,psci-1.0", "arm,psci-0.2"]).and_then(|n| match n.string("method") {
        Some(b"smc") => Some(Conduit::Smc), Some(b"hvc") => Some(Conduit::Hvc), _ => None,
    });
    let rtc = enabled_compatible(tree, &["arm,pl031"]).and_then(|n| Some((n.first_reg()?.0, spi(&n, gic.as_ref()))));
    let ecam = enabled_compatible(tree, &["pci-host-ecam-generic"]).and_then(|n| {
        let (base, size) = n.first_reg()?;
        let range = n.prop("bus-range");
        let (first, last) = match range { Some(r) if r.len() == 8 => (be32(r, 0)?, be32(r, 4)?), None => (0, 255), _ => return None };
        (first <= last && last <= 255 && ((last - first + 1) as u64) << 20 <= size).then_some((base, first as u8, last as u8))
    });
    let framebuffer = enabled_compatible(tree, &["simple-framebuffer"]).and_then(|n| {
        let (base, size) = n.first_reg()?;
        let (width, height, stride) = (n.u32("width")?, n.u32("height")?, n.u32("stride")?);
        let bgr = match n.string("format")? { b"a8r8g8b8" | b"x8r8g8b8" => true, b"a8b8g8r8" | b"x8b8g8r8" => false, _ => return None };
        (width > 0 && height > 0 && stride as u64 >= 4 * width as u64 && size >= stride as u64 * height as u64).then_some(Framebuffer { base, size, width, height, stride, bgr })
    });
    Board { console: console_with(tree, gic.as_ref()), gic, timer, timer_erratum, psci, rtc, ecam, framebuffer }
}

/// Calls `each` with the MPIDR (`reg`) of every enabled CPU of `/cpus` that PSCI starts, in the tree's order;
/// the count of those and of the CPUs it leaves out (another `enable-method`, or a `reg` this reader cannot take).
pub fn cpus(tree: &Tree, mut each: impl FnMut(u64)) -> (usize, usize) {
    let (mut started, mut left) = (0, 0);
    let Some(parent) = tree.at_path(b"/cpus") else { return (0, 0) };
    let address = parent.cells().0;
    tree.walk(|n| {
        if n.is_child_of(&parent) && n.string("device_type") == Some(b"cpu") && n.enabled() {
            let mpidr = n.prop("reg").filter(|r| r.len() >= 4 * address as usize).and_then(|r| cells_at(r, 0, address));
            match (mpidr, n.string("enable-method")) {
                (Some(m), Some(b"psci")) => { each(m); started += 1; }
                _ => left += 1,
            }
        }
        true
    });
    (started, left)
}

/// Calls `each` with every reserved range: the memory reservation block's, then `/reserved-memory`'s children's.
pub fn reserved(tree: &Tree, mut each: impl FnMut(u64, u64)) {
    for (address, size) in tree.reservations() { each(address, size); }
    let Some(parent) = tree.at_path(b"/reserved-memory") else { return };
    tree.walk(|n| { if n.is_child_of(&parent) && n.enabled() { if let Some(reg) = n.reg() { for (a, s) in reg { each(a, s); } } } true });
}
