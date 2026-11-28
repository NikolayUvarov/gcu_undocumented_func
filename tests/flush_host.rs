//! What a flush finds to empty (307-DRV-0055): an ATA drive's IDENTIFY words (`ahci` and `ata`) and a USB disk's MODE
//! SENSE(6) answer (`usb_storage`); an NVMe controller's in tests/nvme_host.rs. A device that says it has no write cache,
//! or that it is off, gets no command; one that says nothing valid gets the flush.
#![allow(dead_code)] // the modules' other parts are driven by their own tests or not on the host
#[path = "../ahci/src/cache.rs"]
mod cache;
#[path = "../usb_storage/src/mode.rs"]
mod mode;

use cache::{Flush, Said, FLUSH_CACHE, FLUSH_CACHE_EXT};

// IDENTIFY words as QEMU's IDE disk gives them with its write cache on (hw/ide/core.c): words 82 to 87.
fn identify(w82: u16, w83: u16, w85: u16, w87: u16) -> impl Fn(usize) -> u16 {
    move |i| match i { 82 => w82, 83 => w83, 84 => 0x4000, 85 => w85, 86 => w83 & 0x3FFF, 87 => w87, _ => 0 }
}

#[test]
fn an_ata_drive_with_its_cache_on_gets_flush_cache_ext_where_it_has_48_bits() {
    assert_eq!(cache::flush(identify(0x4021, 0x7400, 0x4021, 0x4000)), Flush::Send(FLUSH_CACHE_EXT));
    // Without 48-bit addressing, or without FLUSH CACHE EXT, the 28-bit FLUSH CACHE.
    assert_eq!(cache::flush(identify(0x4021, 0x5000, 0x4021, 0x4000)), Flush::Send(FLUSH_CACHE), "no 48-bit addressing");
    assert_eq!(cache::flush(identify(0x4021, 0x5400, 0x4021, 0x4000)), Flush::Send(FLUSH_CACHE), "48 bits, no FLUSH CACHE EXT");
    assert_eq!(cache::flush(identify(0x4021, 0x7000, 0x4021, 0x4000)), Flush::Send(FLUSH_CACHE), "FLUSH CACHE EXT, no 48 bits");
    assert_eq!((cache::name(FLUSH_CACHE_EXT), cache::name(FLUSH_CACHE)), ("FLUSH CACHE EXT", "FLUSH CACHE"));
}

#[test]
fn an_ata_drive_without_a_cache_or_with_it_off_gets_no_command() {
    // QEMU's disk with cache.writeback=off: word 85 bit 5 clear, word 82 still says it has one.
    assert_eq!(cache::flush(identify(0x4021, 0x7400, 0x4001, 0x4000)), Flush::Off);
    assert_eq!(cache::flush(identify(0x4001, 0x7400, 0x4001, 0x4000)), Flush::Absent);
    // Word 82 says none even where word 85 says on.
    assert_eq!(cache::flush(identify(0x4001, 0x7400, 0x4021, 0x4000)), Flush::Absent);
    assert!(cache::why(Flush::Absent).contains("WORD 82") && cache::why(Flush::Off).contains("WORD 85"));
}

#[test]
fn ata_words_not_marked_valid_say_nothing_and_the_flush_is_sent() {
    // Words 82 and 83 count only with word 83's bits 15:14 01; words 85 to 87 only with word 87's.
    assert_eq!(cache::flush(identify(0x0001, 0x0000, 0x4001, 0x4000)), Flush::Off, "word 83 invalid: word 82 ignored, word 85 counts");
    assert_eq!(cache::flush(identify(0x4021, 0x7400, 0x0001, 0x0000)), Flush::Send(FLUSH_CACHE_EXT), "word 87 invalid: word 85 ignored");
    assert_eq!(cache::flush(identify(0x4021, 0x7400, 0x0001, 0xC000)), Flush::Send(FLUSH_CACHE_EXT), "bits 15:14 11 are not valid");
    assert_eq!(cache::flush(|_| 0), Flush::Send(FLUSH_CACHE), "an old drive's zeros");
    assert_eq!(cache::flush(|_| 0xFFFF), Flush::Send(FLUSH_CACHE), "a floating bus's ones");
}

#[test]
fn each_kind_of_flush_line_is_said_once() {
    let mut said = Said::new();
    assert!(said.first(Said::DONE));
    assert!(!said.first(Said::DONE));
    for kind in [Said::NO_CACHE, Said::REFUSED, Said::UNANSWERED] { assert!(said.first(kind)); assert!(!said.first(kind)); }
    assert!(!said.first(Said::DONE));
}

// A MODE SENSE(6) answer: the header, `descriptors` bytes of block descriptors, then `pages`; byte 0 the mode data length.
fn answer(header2: u8, descriptors: usize, pages: &[&Vec<u8>]) -> Vec<u8> {
    let mut data = vec![0, 0, header2, descriptors as u8];
    data.extend(std::iter::repeat(0x11).take(descriptors));
    for page in pages { data.extend_from_slice(page); }
    data[0] = (data.len() - 1) as u8;
    data
}

// Pages as QEMU's SCSI disk gives them for 3Fh: read-write error recovery, rigid disk geometry, flexible disk, caching.
fn caching(byte2: u8) -> Vec<u8> { let mut page = vec![0x08, 0x12, byte2]; page.resize(20, 0); page }
fn page(code: u8, length: u8) -> Vec<u8> { let mut page = vec![code, length]; page.resize(2 + length as usize, 0x04); page }

#[test]
fn a_usb_disks_caching_page_says_whether_its_cache_is_on() {
    let (recovery, geometry, flexible) = (page(0x01, 0x0A), page(0x04, 0x16), page(0x05, 0x1E));
    let on = answer(0x00, 8, &[&recovery, &geometry, &flexible, &caching(0x04)]);
    assert_eq!((mode::write_cache(&on), mode::protected(&on)), (Some(true), Some(false)));
    let off = answer(0x80, 8, &[&recovery, &geometry, &flexible, &caching(0x01)]);
    assert_eq!((mode::write_cache(&off), mode::protected(&off)), (Some(false), Some(true)), "RCD alone is not WCE; WP from the header");
    assert_eq!(mode::write_cache(&answer(0, 0, &[&caching(0x04)])), Some(true), "no block descriptors");
    assert!(on.len() <= mode::LENGTH && mode::all_pages(mode::HEADER) == [0x1A, 0, 0x3F, 0, 4, 0] && mode::all_pages(192) == [0x1A, 0, 0x3F, 0, 192, 0]);
    // The second ask is exactly the length the header names, so that no data stage ends short (bench-54's JMicron).
    assert_eq!(mode::whole(&on[..4]), on.len());
    assert_eq!((mode::whole(&[3, 0, 0, 0]), mode::whole(&[0xFF, 0, 0, 0]), mode::whole(&[0, 0, 0, 0]), mode::whole(&[])), (4, 192, 4, 4));
}

#[test]
fn a_usb_disk_that_gives_no_caching_page_leaves_its_cache_unknown() {
    let recovery = page(0x01, 0x0A);
    assert_eq!(mode::write_cache(&answer(0, 8, &[&recovery])), None, "no caching page");
    assert_eq!(mode::write_cache(&answer(0, 0, &[])), None, "the header alone");
    assert_eq!((mode::write_cache(&[]), mode::protected(&[])), (None, None), "no answer");
    assert_eq!(mode::protected(&[3, 0, 0x80]), None, "a header cut short");
    // A caching page past the answer's end, or past its mode data length, or cut before its byte 2.
    let full = answer(0, 8, &[&recovery, &caching(0x04)]);
    assert_eq!(mode::write_cache(&full[..4 + 8 + 12 + 2]), None, "cut by the transfer");
    let mut short = full.clone();
    short[0] = (4 + 8 + 12 + 2 - 1) as u8;
    assert_eq!(mode::write_cache(&short), None, "cut by the mode data length");
    assert_eq!(mode::write_cache(&full[..4 + 8 + 12 + 3]), Some(true), "its byte 2 is enough");
}

#[test]
fn a_usb_disks_pages_are_walked_by_their_lengths() {
    // A subpage-format page (SPF, a 16-bit length) is skipped whole; one with code 08h is not the caching page.
    let mut subpage = vec![0x48, 0x01, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00];
    assert_eq!(mode::write_cache(&answer(0, 0, &[&subpage])), None);
    subpage[4] = 0x04;
    assert_eq!(mode::write_cache(&answer(0, 0, &[&subpage, &caching(0x00)])), Some(false));
    // Pages of length 0 end the walk at the answer's end.
    let empty = page(0x00, 0);
    assert_eq!(mode::write_cache(&answer(0, 0, &[&empty, &empty, &empty, &caching(0x04)])), Some(true));
    assert_eq!(mode::write_cache(&answer(0, 0, &[&empty; 40])), None);
    // A block descriptor length past the answer.
    assert_eq!(mode::write_cache(&[11, 0, 0, 200, 0x08, 0x12, 0x04, 0, 0, 0, 0, 0]), None);
}
