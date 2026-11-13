// Host tests of usb_storage's look for its device after a lost transfer (000-DRV-0058): the production logic against a
// model of usb_host's claims, which hands out the free interfaces in its table's order, as usb_host does.
#[path = "../usb_storage/src/recovery.rs"]
mod recovery;

use recovery::{reclaim, verdict, Answer, Claim, Found, Port, Verdict, Why, HELD};

// One interface in the model's table: its handle, whether it is SCSI over Bulk-Only, its medium's sectors (0: none), the
// round from which usb_host offers it (a device still being set up), and whether a client holds it.
struct Entry { handle: u32, bulk_only: bool, medium: u64, from: usize, held: bool }

// usb_host as the driver sees it after giving its old claim back; `new_host`: the next claim finds a new instance.
struct Model { table: Vec<Entry>, round: usize, new_host: bool, released: Vec<u32>, asked: Vec<u32> }

impl Model {
    fn new(table: Vec<Entry>) -> Self { Model { table, round: 0, new_host: false, released: vec![], asked: vec![] } }
    fn held(&self) -> Vec<u32> { self.table.iter().filter(|e| e.held).map(|e| e.handle).collect() }
}

impl Port<u32> for Model {
    fn claim(&mut self) -> Claim<u32> {
        if self.new_host { self.new_host = false; return Claim::NewHost; }
        let round = self.round;
        match self.table.iter_mut().find(|e| !e.held && e.from <= round) {
            Some(entry) => { entry.held = true; Claim::Interface(entry.handle, entry.bulk_only.then_some(entry.handle)) }
            None => Claim::None,
        }
    }
    fn release(&mut self, handle: u32) {
        self.released.push(handle);
        if let Some(entry) = self.table.iter_mut().find(|e| e.handle == handle) { entry.held = false; }
    }
    fn ready(&mut self, handle: &mut u32) -> Option<u64> {
        self.asked.push(*handle);
        self.table.iter().find(|e| e.handle == *handle && e.medium != 0).map(|e| e.medium)
    }
    fn pause(&mut self) { self.round += 1; }
}

const SECTORS: u64 = 1_425_408;
fn stick(handle: u32) -> Entry { Entry { handle, bulk_only: true, medium: SECTORS, from: 0, held: false } }
fn reader(handle: u32) -> Entry { Entry { handle, bulk_only: true, medium: 0, from: 0, held: false } }
fn other_class(handle: u32) -> Entry { Entry { handle, bulk_only: false, medium: SECTORS, from: 0, held: false } }
fn later(mut entry: Entry, round: usize) -> Entry { entry.from = round; entry }

// Handles as usb_host makes them: generation << 16 | device byte << 8 | interface.
const OURS: u32 = 1 << 16 | 0x01 << 8;
const AGAIN: u32 = 3 << 16 | 0x01 << 8; // the same port enumerated again: a later generation
const SECOND: u32 = 2 << 16 | 0x02 << 8;

#[test]
fn a_device_that_only_missed_a_transfer_is_the_same() {
    let mut usb = Model::new(vec![stick(OURS)]);
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Same, false));
    assert_eq!(usb.held(), vec![OURS], "the same interface stays claimed");
    assert!(usb.asked.is_empty() && usb.round == 0, "found at the first claim, nothing else asked");
}

#[test]
fn the_same_one_is_found_behind_another_stick() {
    // Another stick with a medium comes first in usb_host's table: it is not taken for a return.
    let mut usb = Model::new(vec![stick(SECOND), reader(SECOND + 1), stick(OURS)]);
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Same, false));
    assert_eq!(usb.held(), vec![OURS]);
    assert!(usb.asked.is_empty(), "no other device asked for its medium");
}

#[test]
fn a_device_enumerated_again_came_back() {
    let mut usb = Model::new(vec![stick(AGAIN)]);
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Other(SECTORS), false));
    assert!(usb.held().is_empty(), "given back for the fresh driver instance");
    assert_eq!(usb.asked, vec![AGAIN]);
}

#[test]
fn a_device_still_being_set_up_is_waited_for() {
    let mut usb = Model::new(vec![later(stick(AGAIN), 3)]);
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Other(SECTORS), false));
    assert_eq!(usb.round, 3);
    assert!(usb.held().is_empty());
}

#[test]
fn a_gone_device_gets_every_try() {
    let mut usb = Model::new(vec![]);
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Nothing, false));
    assert_eq!(usb.round, 19, "19 pauses between 20 rounds: 2 s that vfs_server sees as a drive that does not answer");
}

#[test]
fn an_empty_reader_or_another_class_is_no_return() {
    let mut usb = Model::new(vec![reader(SECOND), other_class(SECOND + 1)]);
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Nothing, false));
    assert_eq!(usb.asked, vec![SECOND], "the reader asked once, the other class never");
    assert!(usb.held().is_empty(), "both given back");
}

#[test]
fn a_stick_behind_an_empty_reader_came_back() {
    let mut usb = Model::new(vec![reader(SECOND), later(stick(AGAIN), 2)]);
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Other(SECTORS), false));
    assert_eq!(usb.asked, vec![SECOND, AGAIN]);
    assert!(usb.held().is_empty());
}

#[test]
fn a_new_usb_host_numbers_from_the_start() {
    // A new instance hands out the old handle's number for its own first device: not the same one.
    let mut usb = Model::new(vec![stick(OURS)]);
    usb.new_host = true;
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Other(SECTORS), true));
    assert!(usb.held().is_empty());
}

#[test]
fn after_a_peer_no_handle_is_the_same() {
    let mut usb = Model::new(vec![stick(OURS)]);
    assert_eq!(reclaim(None, 20, &mut usb), (Found::Other(SECTORS), false));
}

#[test]
fn a_full_table_ends_without_claiming_its_own_again() {
    let mut usb = Model::new((0..HELD as u32 + 3).map(|i| reader(SECOND + i)).collect());
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Nothing, false));
    assert_eq!(usb.asked.len(), HELD);
    assert_eq!(usb.round, 0, "no round can claim more");
    assert!(usb.held().is_empty());
    assert_eq!(usb.released.len(), HELD);
}

#[test]
fn another_capacity_is_found_and_told() {
    let mut usb = Model::new(vec![Entry { medium: 2 * SECTORS, ..stick(AGAIN) }]);
    assert_eq!(reclaim(Some(OURS), 20, &mut usb), (Found::Other(2 * SECTORS), false));
    assert!(usb.held().is_empty());
}

#[test]
fn the_verdicts() {
    let same = |sectors, changed| Some(Answer { sectors, changed });
    assert_eq!(verdict(Found::Same, false, 1000, same(1000, false)), Verdict::Back);
    assert_eq!(verdict(Found::Same, false, 1000, same(1000, true)), Verdict::Returned(Why::MediumChanged));
    assert_eq!(verdict(Found::Same, false, 1000, None), Verdict::Gone);
    assert_eq!(verdict(Found::Other(1000), false, 1000, None), Verdict::Returned(Why::Enumerated));
    assert_eq!(verdict(Found::Other(1000), true, 1000, None), Verdict::Returned(Why::NewHost));
    assert_eq!(verdict(Found::Nothing, true, 1000, None), Verdict::Gone);
    // Another capacity is not used, the same interface's or another one's, until vfs_server checks a return itself.
    assert_eq!(verdict(Found::Same, false, 1000, same(2000, true)), Verdict::Unused(2000));
    assert_eq!(verdict(Found::Other(2000), false, 1000, None), Verdict::Unused(2000));
    assert_eq!(verdict(Found::Other(2000), true, 1000, None), Verdict::Unused(2000));
    // Each return is logged with what tests/usb_image_smoke.py looks for.
    for why in [Why::Enumerated, Why::NewHost, Why::MediumChanged] { assert!(why.text().contains("CAME BACK"), "{:?}", why); }
}
