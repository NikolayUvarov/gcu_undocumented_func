#[path = "../rtl8168/src/identity.rs"]
mod identity;
#[path = "../rtl8168/src/rings.rs"]
mod rings;

use rings::{DropReason, Received, RegionError, Rings, SendError};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ptr;

const PHYSICAL: u64 = 0x1200_0000;

struct Dma {
    base: *mut u8,
}

impl Dma {
    fn new() -> Self {
        let base = unsafe { alloc_zeroed(Layout::from_size_align(rings::DMA_BYTES, 256).unwrap()) };
        assert!(!base.is_null());
        Self { base }
    }

    fn initialize(&self) -> Rings {
        unsafe { Rings::initialize(self.base, rings::DMA_BYTES, PHYSICAL).unwrap() }
    }

    fn word(&self, offset: usize) -> u32 {
        assert!(offset + 4 <= rings::DMA_BYTES && offset % 4 == 0);
        unsafe { u32::from_le(ptr::read_volatile(self.base.add(offset).cast())) }
    }

    fn set_word(&self, offset: usize, value: u32) {
        assert!(offset + 4 <= rings::DMA_BYTES && offset % 4 == 0);
        unsafe { ptr::write_volatile(self.base.add(offset).cast(), value.to_le()) }
    }

    fn bytes(&self, offset: usize, length: usize) -> Vec<u8> {
        assert!(offset + length <= rings::DMA_BYTES);
        (0..length)
            .map(|i| unsafe { ptr::read_volatile(self.base.add(offset + i)) })
            .collect()
    }

    fn rx(&self, slot: usize, status: u32, payload: &[u8]) {
        assert!(slot < 256 && payload.len() <= 2048);
        let descriptor = 4096 + slot * 16;
        assert_ne!(self.word(descriptor) & 0x8000_0000, 0);
        unsafe {
            let buffer = self.base.add(532480 + slot * 2048);
            ptr::write_bytes(buffer, 0xcc, 2048);
            ptr::copy_nonoverlapping(payload.as_ptr(), buffer, payload.len());
        }
        self.set_word(descriptor + 4, 0xdead_beef);
        self.set_word(
            descriptor,
            status | if slot == 255 { 0x4000_0000 } else { 0 },
        );
    }

    fn tx_done(&self, slot: usize, status: u32) {
        assert!(slot < 256);
        let previous = self.word(slot * 16);
        assert_ne!(previous & 0x8000_0000, 0);
        self.set_word(slot * 16, previous & !0x8000_0000 | status);
    }
}

impl Drop for Dma {
    fn drop(&mut self) {
        unsafe {
            dealloc(
                self.base,
                Layout::from_size_align(rings::DMA_BYTES, 256).unwrap(),
            )
        }
    }
}

#[test]
fn initialization_matches_wire_layout_alignment_and_last_descriptor_only() {
    let dma = Dma::new();
    let queues = dma.initialize();
    assert_eq!(rings::DMA_BYTES, 1056768);
    assert_eq!(queues.tx_address(), PHYSICAL);
    assert_eq!(queues.rx_address(), PHYSICAL + 4096);
    assert_eq!(queues.tx_pending(), 0);
    for slot in 0..256 {
        let eor = if slot == 255 { 0x4000_0000 } else { 0 };
        assert_eq!(dma.word(slot * 16), eor);
        assert_eq!(dma.word(slot * 16 + 4), 0);
        assert_eq!(
            dma.word(slot * 16 + 8),
            PHYSICAL as u32 + 8192 + slot as u32 * 2048
        );
        assert_eq!(dma.word(slot * 16 + 12), 0);
        let rx = 4096 + slot * 16;
        assert_eq!(dma.word(rx), 0x8000_0800 | eor);
        assert_eq!(dma.word(rx + 4), 0);
        assert_eq!(
            dma.word(rx + 8),
            PHYSICAL as u32 + 532480 + slot as u32 * 2048
        );
        assert_eq!(dma.word(rx + 12), 0);
    }
    assert_eq!(
        dma.bytes(0, 16),
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0x20, 0, 0x12, 0, 0, 0, 0]
    );
}

#[test]
fn invalid_dma_regions_are_rejected_without_writes() {
    let dma = Dma::new();
    unsafe { ptr::write_bytes(dma.base, 0xa5, rings::DMA_BYTES) };
    let cases = [
        (
            ptr::null_mut(),
            rings::DMA_BYTES,
            PHYSICAL,
            RegionError::Null,
        ),
        (
            unsafe { dma.base.add(1) },
            rings::DMA_BYTES - 1,
            PHYSICAL,
            RegionError::Alignment,
        ),
        (
            dma.base,
            rings::DMA_BYTES,
            PHYSICAL + 1,
            RegionError::Alignment,
        ),
        (
            dma.base,
            rings::DMA_BYTES - 1,
            PHYSICAL,
            RegionError::TooSmall,
        ),
        (dma.base, rings::DMA_BYTES, 0, RegionError::AddressRange),
        (
            dma.base,
            rings::DMA_BYTES,
            0xffff_ff00,
            RegionError::AddressRange,
        ),
        (
            dma.base,
            rings::DMA_BYTES,
            u64::MAX - 255,
            RegionError::AddressRange,
        ),
    ];
    for (base, bytes, physical, expected) in cases {
        assert_eq!(
            unsafe { Rings::initialize(base, bytes, physical) }.err(),
            Some(expected)
        );
        assert_eq!(dma.bytes(0, 32), vec![0xa5; 32]);
    }
    let physical = (1u64 << 32) - rings::DMA_BYTES as u64;
    assert!(unsafe { Rings::initialize(dma.base, rings::DMA_BYTES, physical) }.is_ok());
}

#[test]
fn tx_pads_short_frames_rejects_lengths_and_keeps_dma_owned_buffers_untouched() {
    let dma = Dma::new();
    let mut queues = dma.initialize();
    for size in [0, 13, 1515, 2048] {
        assert_eq!(queues.send(&vec![0xab; size]), Err(SendError::FrameLength));
        assert_eq!(queues.tx_pending(), 0);
    }
    assert_eq!(queues.send(&[0xab; 14]), Ok(0));
    assert_eq!(dma.word(0), 0xb000_003c);
    assert_eq!(dma.word(4), 0);
    assert_eq!(dma.bytes(8192, 14), vec![0xab; 14]);
    assert_eq!(dma.bytes(8192 + 14, 46), vec![0; 46]);
    assert_eq!(queues.reclaim(), None);
    assert_eq!(queues.tx_pending(), 1);
    dma.set_word(16, 0x8000_0000);
    let before = dma.bytes(8192 + 2048, 64);
    assert_eq!(queues.send(&[0xcd; 64]), Err(SendError::DeviceOwned));
    assert_eq!(dma.bytes(8192 + 2048, 64), before);
    assert_eq!(queues.tx_pending(), 1);
}

#[test]
fn tx_full_ring_reclaims_in_order_and_wraps_without_overwriting_pending_slots() {
    let dma = Dma::new();
    let mut queues = dma.initialize();
    for slot in 0..256 {
        assert_eq!(queues.send(&[slot as u8; 1514]), Ok(slot));
    }
    assert_eq!(dma.word(255 * 16), 0xf000_05ea);
    assert_eq!(queues.tx_pending(), 256);
    assert_eq!(queues.send(&[0x11; 60]), Err(SendError::Full));
    dma.tx_done(1, 0);
    assert_eq!(queues.reclaim(), None);
    dma.tx_done(0, 0x0080_0000);
    let completion = queues.reclaim().unwrap();
    assert_eq!(completion.slot, 0);
    assert_eq!(completion.status, 0x3080_05ea);
    assert_eq!(queues.tx_pending(), 255);
    assert_eq!(queues.send(&[0xee; 60]), Ok(0));
    assert_eq!(queues.send(&[0xff; 60]), Err(SendError::Full));
    for slot in 1..256 {
        if slot != 1 {
            dma.tx_done(slot, 0);
        }
        assert_eq!(queues.reclaim().unwrap().slot, slot);
    }
    assert_eq!(queues.reclaim(), None);
    assert_eq!(queues.tx_pending(), 1);
    dma.tx_done(0, 0);
    assert_eq!(queues.reclaim().unwrap().slot, 0);
    assert_eq!(queues.reclaim(), None);
    assert_eq!(dma.word(255 * 16), 0x4000_0000);
    assert_eq!(queues.tx_pending(), 0);
}

#[test]
fn rx_keeps_ownership_until_output_fits_then_copies_and_strips_fcs() {
    let dma = Dma::new();
    let mut queues = dma.initialize();
    let mut small = [0xa5; 59];
    assert_eq!(queues.receive(&mut small), Received::Empty);
    assert_eq!(small, [0xa5; 59]);
    dma.rx(0, 0x3000_0040, &[0x5a; 60]);
    assert_eq!(
        queues.receive(&mut small),
        Received::OutputTooSmall { needed: 60 }
    );
    assert_eq!(small, [0xa5; 59]);
    assert_eq!(dma.word(4096), 0x3000_0040);
    assert_eq!(dma.word(4100), 0xdead_beef);
    let mut output = [0xa5; 100];
    assert_eq!(queues.receive(&mut output), Received::Frame(60));
    assert_eq!(output[..60], [0x5a; 60]);
    assert_eq!(output[60..], [0xa5; 40]);
    assert_eq!(dma.word(4096), 0x8000_0800);
    assert_eq!(dma.word(4100), 0);
    assert_eq!(queues.receive(&mut output), Received::Empty);
}

#[test]
fn rx_rejects_bad_descriptors_one_per_call_without_touching_output() {
    let dma = Dma::new();
    let mut queues = dma.initialize();
    let cases = [
        (0x3020_0040, DropReason::Hardware),
        (0x3040_0040, DropReason::Hardware),
        (0x3010_0040, DropReason::Hardware),
        (0x3008_0040, DropReason::Hardware),
        (0x1000_0040, DropReason::Fragment),
        (0x2000_0040, DropReason::Fragment),
        (0x0000_0040, DropReason::Fragment),
        (0x3000_0000, DropReason::Length),
        (0x3000_0003, DropReason::Length),
        (0x3000_003f, DropReason::Length),
        (0x3000_05ef, DropReason::Length),
        (0x3000_3fff, DropReason::Length),
    ];
    for (slot, &(status, _)) in cases.iter().enumerate() {
        dma.rx(slot, status, &[0x5a; 60]);
    }
    let mut output = [0xa5; 1514];
    for (slot, &(status, reason)) in cases.iter().enumerate() {
        assert_eq!(
            queues.receive(&mut output),
            Received::Dropped { reason, status }
        );
        assert_eq!(output, [0xa5; 1514]);
        assert_eq!(dma.word(4096 + slot * 16), 0x8000_0800);
        if slot + 1 < cases.len() {
            assert_eq!(dma.word(4096 + (slot + 1) * 16) & 0x8000_0000, 0);
        }
    }
    assert_eq!(queues.receive(&mut output), Received::Empty);
}

#[test]
fn rx_max_frames_wrap_and_preserve_last_descriptor_marker() {
    let dma = Dma::new();
    let mut queues = dma.initialize();
    let mut output = [0; 1514];
    for turn in 0..513 {
        let slot = turn % 256;
        dma.rx(slot, 0x3000_05ee, &[turn as u8; 1514]);
        assert_eq!(queues.receive(&mut output), Received::Frame(1514));
        assert_eq!(output, [turn as u8; 1514]);
        let expected = if slot == 255 {
            0xc000_0800
        } else {
            0x8000_0800
        };
        assert_eq!(dma.word(4096 + slot * 16), expected);
    }
    assert_eq!(queues.receive(&mut output), Received::Empty);
}

#[test]
fn identity_decodes_revision_from_raw_registers_without_claiming_chip_support() {
    for (register, expected) in [
        (0x5410_0780, 0x541),
        (0x5c80_0780, 0x5c8),
        (0x54f0_0780, 0x54f),
    ] {
        let chip = identity::Identity::decode(0x8168_10ec, register).unwrap();
        assert_eq!(chip.tx_config, register);
        assert_eq!(chip.xid, expected);
    }
    let mut registers = [0; 256];
    registers[64..68].copy_from_slice(&[0x80, 0x07, 0x10, 0x54]);
    assert_eq!(
        identity::Identity::from_snapshot(identity::PCI_ID, &registers)
            .unwrap()
            .xid,
        0x541
    );
    for length in 0..68 {
        assert_eq!(
            identity::Identity::from_snapshot(identity::PCI_ID, &registers[..length]),
            Err(identity::Error::Truncated)
        );
    }
    for register in [0, u32::MAX, 0x0000_0780] {
        assert_eq!(
            identity::Identity::decode(identity::PCI_ID, register),
            Err(identity::Error::Unavailable)
        );
    }
    assert_eq!(
        identity::Identity::decode(0xb852_10ec, 0x5410_0780),
        Err(identity::Error::WrongDevice)
    );
    assert_eq!(
        identity::Identity::from_snapshot(0x8139_10ec, &[]),
        Err(identity::Error::WrongDevice)
    );
}
