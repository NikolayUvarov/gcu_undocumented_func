// The platform's nodes the kernel lends (100000-KRN-0171): one table, filled once on the boot CPU before the others start
// (devicetree.rs from the tree, else platform.rs from the firmware's tables) and only read after that.
use crate::nodes::Nodes;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

struct Table(UnsafeCell<Nodes>);
unsafe impl Sync for Table {} // written once before any other CPU runs, read only after
static TABLE: Table = Table(UnsafeCell::new(Nodes::new()));
static BUILT: AtomicBool = AtomicBool::new(false);

/// Fills the table, once: a later call does nothing. Only on the boot CPU, before the others start.
pub unsafe fn build(fill: impl FnOnce(&mut Nodes)) {
    if BUILT.load(Ordering::Acquire) { return; }
    fill(&mut *TABLE.0.get());
    BUILT.store(true, Ordering::Release);
}
pub fn table() -> &'static Nodes { unsafe { &*TABLE.0.get() } }
