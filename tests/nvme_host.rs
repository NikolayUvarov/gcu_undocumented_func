//! `nvme`'s instance rule (213-DRV-0060): init's argument names the instance, the first alone writes, and any other one
//! sends its controller only Identify, the I/O queues' bring-up (Set Features for their number alone, unsaved; their
//! creation) and Read; every other opcode of either queue is refused, the medium-changing ones named among them. Refusals
//! are said one line each up to a bound, then counted. A flush finds no cache to empty without VWC, or with Get Features
//! saying it is off (307-DRV-0055).
#[path = "../nvme/src/guard.rs"]
mod guard;

use guard::{Queue, Refusals, Tag};

#[test]
fn the_argument_names_the_instance() {
    assert_eq!(guard::instance(""), Some(0), "init starts the first instance without an argument");
    assert_eq!(guard::instance("0"), Some(0));
    assert_eq!(guard::instance(" 1\n"), Some(1));
    assert_eq!(guard::instance("2"), Some(2));
    for other in ["x", "-1", "+1", "1 2", "1x", "0x1", "99999999999"] {
        assert_eq!(guard::instance(other), None, "{other:?} names no instance");
    }
}

#[test]
fn only_the_first_instance_writes() {
    assert!(guard::writes(Some(0)));
    assert!(!guard::writes(Some(1)));
    assert!(!guard::writes(Some(7)));
    assert!(!guard::writes(None), "an argument that names no instance is read-only");
    assert!(!guard::writes(guard::instance("nvme#1")));
}

#[test]
fn the_first_instance_sends_what_it_always_sent() {
    for opcode in 0..=u8::MAX {
        assert!(guard::allowed(true, Queue::Admin, opcode, 0));
        assert!(guard::allowed(true, Queue::Io, opcode, 0));
    }
}

#[test]
fn a_read_only_instance_sends_identify_queue_creation_and_read_alone() {
    // With any dword 10 but the Number of Queues', Set Features is refused with the rest.
    for cdw10 in [0, 0x01, 0x06, 0x08, 0x0C, 0x8000_0007, 0x0107] {
        let admin: Vec<u8> = (0..=u8::MAX).filter(|&op| guard::allowed(false, Queue::Admin, op, cdw10)).collect();
        let io: Vec<u8> = (0..=u8::MAX).filter(|&op| guard::allowed(false, Queue::Io, op, cdw10)).collect();
        assert_eq!(admin, [guard::OP_CREATE_SQ, guard::OP_CREATE_CQ, guard::OP_IDENTIFY], "dword 10 {cdw10:#x}");
        assert_eq!(io, [guard::OP_READ], "dword 10 {cdw10:#x}");
    }
    let admin: Vec<u8> = (0..=u8::MAX).filter(|&op| guard::allowed(false, Queue::Admin, op, guard::QUEUES_FEATURE)).collect();
    assert_eq!(admin, [guard::OP_CREATE_SQ, guard::OP_CREATE_CQ, guard::OP_IDENTIFY, guard::OP_SET_FEATURES], "the number of queues, unsaved");
    let io: Vec<u8> = (0..=u8::MAX).filter(|&op| guard::allowed(false, Queue::Io, op, guard::QUEUES_FEATURE)).collect();
    assert_eq!(io, [guard::OP_READ], "no I/O command by the queues' dword");
    assert!(!guard::allowed(false, Queue::Io, guard::OP_WRITE, 0) && !guard::allowed(false, Queue::Io, guard::OP_FLUSH, 0));
    // What changes the medium or the controller's kept state, by name (NVMe 1.4, figures 139 and 346).
    for (queue, opcode, name) in [
        (Queue::Io, 0x00, "FLUSH"), (Queue::Io, 0x01, "WRITE"), (Queue::Io, 0x04, "WRITE UNCORRECTABLE"), (Queue::Io, 0x08, "WRITE ZEROES"),
        (Queue::Io, 0x09, "DATASET MANAGEMENT"), (Queue::Io, 0x0D, "RESERVATION REGISTER"), (Queue::Io, 0x11, "RESERVATION ACQUIRE"),
        (Queue::Admin, 0x09, "SET FEATURES"), (Queue::Admin, 0x0D, "NAMESPACE MANAGEMENT"), (Queue::Admin, 0x15, "NAMESPACE ATTACHMENT"),
        (Queue::Admin, 0x10, "FIRMWARE COMMIT"), (Queue::Admin, 0x11, "FIRMWARE IMAGE DOWNLOAD"), (Queue::Admin, 0x80, "FORMAT NVM"),
        (Queue::Admin, 0x84, "SANITIZE"), (Queue::Admin, 0x81, "SECURITY SEND"), (Queue::Admin, 0x14, "DEVICE SELF-TEST"),
        (Queue::Admin, 0x02, "GET LOG PAGE"), (Queue::Admin, 0x0C, "ASYNCHRONOUS EVENT REQUEST"), (Queue::Admin, 0x00, "DELETE I/O SUBMISSION QUEUE"),
    ] {
        assert!(!guard::allowed(false, queue, opcode, 0), "{name} sent by a read-only instance");
        assert_eq!(guard::name(queue, opcode), name);
    }
    assert_eq!(guard::name(Queue::Io, 0x7F), "ANOTHER COMMAND");
}

#[test]
fn the_tag_tells_the_instances_apart() {
    assert_eq!(Tag(Some(0)).to_string(), "[NVME]", "the first instance's lines as before");
    assert_eq!(Tag(Some(1)).to_string(), "[NVME#1]");
    assert_eq!(Tag(None).to_string(), "[NVME#?]");
}

#[test]
fn refusals_are_said_then_counted() {
    let mut refusals = Refusals::new();
    let said: Vec<u64> = (0..5000).filter_map(|_| refusals.one()).collect();
    let mut expected: Vec<u64> = (1..=guard::SAID).collect();
    expected.extend([1024, 2048, 3072, 4096]);
    assert_eq!(said, expected);
    assert!(!guard::after(guard::SAID).is_empty(), "the last line said tells that later ones are counted");
    assert_eq!(guard::after(1), "");
    assert_eq!(guard::after(1024), "");
}

#[test]
fn identify_strings_are_printable_and_trimmed() {
    let mut out = [0u8; 40];
    let mut model = [b' '; 40];
    model[..20].copy_from_slice(b"Netac NVMe SSD 128GB");
    assert_eq!(guard::ascii(&model, &mut out), "Netac NVMe SSD 128GB");
    let mut odd = [0u8; 8];
    odd[..4].copy_from_slice(&[b'V', 0x07, b'1', 0xC3]);
    assert_eq!(guard::ascii(&odd, &mut out), "V?1?", "control and non-ASCII bytes shown as ?, trailing NULs cut");
    assert_eq!(guard::ascii(&[b' '; 8], &mut out), "");
}

#[test]
fn an_nvme_controller_without_vwc_or_with_it_off_has_no_cache_to_empty() {
    assert_eq!(guard::cache(0, None), guard::Cache::Absent);
    assert_eq!(guard::cache(0, Some(1)), guard::Cache::Absent, "no VWC: Get Features is not believed");
    assert_eq!(guard::cache(0x06, None), guard::Cache::Absent, "bits 2:1 are the broadcast NSID's support, not a cache");
    assert_eq!(guard::cache(1, Some(0)), guard::Cache::Off);
    assert_eq!(guard::cache(0x07, Some(0xFFFF_FFFE)), guard::Cache::Off, "WCE is dword 0's bit 0 alone");
    assert_eq!(guard::cache(1, Some(1)), guard::Cache::On);
    assert_eq!(guard::cache(1, None), guard::Cache::On, "a cache not asked of, or with no answer, is flushed");
    assert_eq!((guard::OP_GET_FEATURES, guard::VWC_FEATURE), (0x0A, 0x06));
    assert!(!guard::allowed(false, guard::Queue::Admin, guard::OP_GET_FEATURES, guard::VWC_FEATURE), "a read-only instance never asks");
}
