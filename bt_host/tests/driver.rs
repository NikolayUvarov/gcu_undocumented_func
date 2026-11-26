//! bt_hci's whole start and loop (bt_host::driver, which bt_hci runs through its usb_host port) against a scripted
//! controller: time advanced by the harness, commands in, events out in the interrupt endpoint's 16-byte packets.
//! An RTL8821C-like and an RTL8852B-like controller, patched from constructed files; the quiet path (nothing listed,
//! no listening) against main's bt_hci's sequence; the active path's polling, and a controller that answers nothing,
//! answers errors or floods events. Evidence for the driver's logic only: a real controller is the hardware's (MC-12.1).
use bt_host::buf::Addr;
use bt_host::driver::{self, Bring, Firmware, Next, Port, PortError};
use bt_host::hci;
use bt_host::rtl;
use bt_host::stack::Stack;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Chip { C8821, C8852 }
/// How the controller answers the commands after the bring-up's (the stack's setup and later).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Answers { Normal, Errors, Nothing, Flood }

/// The bring-up's commands, which every controller answers.
const BRING_UP: [u16; 5] = [hci::RESET, hci::READ_LOCAL_VERSION, rtl::READ_ROM_VERSION, rtl::DOWNLOAD, hci::READ_BD_ADDR];
const ADDRESS: [u8; 6] = [0xA6, 0x6B, 0xF0, 0x2D, 0x79, 0x1C];

struct Fake {
    chip: Chip, answers: Answers, now: u64, patched: bool, downloads: usize,
    queue: VecDeque<Vec<u8>>, commands: Vec<(u64, u16, usize)>, lines: Vec<String>,
    event_polls: Vec<u64>, acl_polls: Vec<u64>, acl_out: usize, slept: u64, quiet_after_bring_up: bool,
}

fn epatch() -> Vec<u8> {
    let mut f = b"Realtech".to_vec();
    f.extend_from_slice(&0x75B8_F098u32.to_le_bytes());
    f.extend_from_slice(&2u16.to_le_bytes());
    for id in [1u16, 2] { f.extend_from_slice(&id.to_le_bytes()); }
    for len in [8u16, 600] { f.extend_from_slice(&len.to_le_bytes()); }
    for at in [0x30u32, 0x40] { f.extend_from_slice(&at.to_le_bytes()); }
    f.resize(0x30, 0);
    f.extend_from_slice(&[0xA1; 8]);
    f.resize(0x40, 0);
    f.extend(std::iter::repeat(0xB2).take(600));
    f.extend_from_slice(&[0xFF, 10, 1, 0, 0x51, 0x04, 0xFD, 0x77]);
    f
}

/// An "RTBTCore" file with one patch section: ROM version 0's part of 300 bytes.
fn core() -> Vec<u8> {
    let mut f = b"RTBTCore".to_vec();
    f.extend_from_slice(&0x0002_33C4_0135_01F1u64.to_le_bytes());
    f.extend_from_slice(&1u32.to_le_bytes());
    let mut body = 1u16.to_le_bytes().to_vec();
    body.extend_from_slice(&[0, 0, 1, 1, 0, 0]);
    body.extend_from_slice(&300u32.to_le_bytes());
    body.extend(std::iter::repeat(0xC3).take(300));
    f.extend_from_slice(&1u32.to_le_bytes());
    f.extend_from_slice(&(body.len() as u32).to_le_bytes());
    f.extend_from_slice(&body);
    f.extend_from_slice(&[0, 0, 0, 0xFF, 20, 1, 0, 0x51, 0x04, 0xFD, 0x77]);
    f
}

impl Fake {
    fn new(chip: Chip, answers: Answers) -> Self {
        Self { chip, answers, now: 2_800, patched: false, downloads: 0, queue: VecDeque::new(), commands: Vec::new(), lines: Vec::new(),
               event_polls: Vec::new(), acl_polls: Vec::new(), acl_out: 0, slept: 0, quiet_after_bring_up: false }
    }
    fn complete(&mut self, op: u16, ret: &[u8]) {
        let mut ev = vec![hci::EV_COMMAND_COMPLETE, (3 + ret.len()) as u8, 1, op as u8, (op >> 8) as u8];
        ev.extend_from_slice(ret);
        self.queue.push_back(ev);
    }
    fn version(&self) -> Vec<u8> {
        let (hci, rev, subver) = match (self.chip, self.patched) {
            (Chip::C8821, false) => (8, 0x000C, 0x8821), (Chip::C8821, true) => (8, 0x75B8, 0xF098),
            (Chip::C8852, false) => (11, 0x000B, 0x8852), (Chip::C8852, true) => (11, 0x42D3, 0x4E04),
        };
        let (r, s) = (u16::to_le_bytes(rev), u16::to_le_bytes(subver));
        vec![0, hci, r[0], r[1], hci, 0x5D, 0x00, s[0], s[1]]
    }
    fn ops(&self) -> Vec<u16> { self.commands.iter().map(|c| c.1).collect() }
    fn logged(&self, needle: &str) -> bool { self.lines.iter().any(|l| l.contains(needle)) }
}

impl Port for Fake {
    fn now(&mut self) -> u64 { self.now }
    fn sleep(&mut self, ms: u64) { self.now += ms; self.slept += ms; }
    fn command(&mut self, packet: &[u8]) -> Result<(), PortError> {
        let op = u16::from_le_bytes([packet[0], packet[1]]);
        assert_eq!(packet[2] as usize, packet.len() - 3);
        self.commands.push((self.now, op, packet.len()));
        let bring_up = BRING_UP.contains(&op) || (self.quiet_after_bring_up && matches!(op, hci::SET_EVENT_MASK | hci::WRITE_INQUIRY_MODE));
        match (bring_up, self.answers) {
            (false, Answers::Nothing) => return Ok(()),
            (false, Answers::Errors) => { self.complete(op, &[0x01]); return Ok(()); }
            _ => {}
        }
        match op {
            hci::READ_LOCAL_VERSION => { let v = self.version(); self.complete(op, &v); }
            rtl::READ_ROM_VERSION => self.complete(op, &[0, if self.chip == Chip::C8821 { 1 } else { 0 }]),
            rtl::DOWNLOAD => { self.downloads += 1; self.complete(op, &[0, packet[3]]); }
            hci::RESET => { if self.downloads > 0 { self.patched = true; } self.complete(op, &[0]); }
            hci::READ_BD_ADDR => { let mut r = vec![0]; r.extend_from_slice(&ADDRESS); self.complete(op, &r); }
            hci::READ_BUFFER_SIZE => self.complete(op, &[0, 0xFD, 0x03, 0xFF, 8, 0, 0, 0]),
            hci::READ_LOCAL_FEATURES => self.complete(op, &[0, 0xFF, 0xFF, 0xFF, 0xFE, 0xDB, 0xFF, 0x7B, 0x87]),
            hci::LE_READ_BUFFER_SIZE => self.complete(op, &[0, 251, 0, 8]),
            hci::LE_SET_SCAN_PARAMETERS | hci::LE_SET_SCAN_ENABLE => self.complete(op, &[0]),
            hci::INQUIRY => self.queue.push_back(vec![hci::EV_COMMAND_STATUS, 4, 0, 1, op as u8, (op >> 8) as u8]),
            _ => self.complete(op, &[0]),
        }
        Ok(())
    }
    fn events(&mut self, each: &mut dyn FnMut(&[u8])) -> Result<(), PortError> {
        self.event_polls.push(self.now);
        if self.answers == Answers::Flood {
            // Eight vendor events a poll, beside the answers.
            for _ in 0..8 { each(&[0xFF, 2, 0x37, 0x01]); }
        }
        // usb_host gives at most eight packets a call, each at most the endpoint's 16 bytes.
        let mut given = 0;
        while given < 8 {
            let Some(ev) = self.queue.front_mut() else { break };
            let n = ev.len().min(16);
            each(&ev[..n].to_vec());
            ev.drain(..n);
            if ev.is_empty() { self.queue.pop_front(); }
            given += 1;
        }
        Ok(())
    }
    fn acl_in(&mut self, _each: &mut dyn FnMut(&[u8])) -> Result<(), PortError> { self.acl_polls.push(self.now); Ok(()) }
    fn acl_out(&mut self, _packet: &[u8]) -> Result<(), PortError> { self.acl_out += 1; Ok(()) }
    fn say(&mut self, line: core::fmt::Arguments) { self.lines.push(line.to_string()); }
    fn firmware(&mut self, which: Firmware, out: &mut [u8]) -> Option<usize> {
        let f = match which { Firmware::Rtl8821cPatch => epatch(), Firmware::Rtl8821cConfig => vec![0x55, 0xAB, 0x23, 0x87, 4, 0, 0xD9, 0, 1, 0x0F], Firmware::Rtl8852bPatch => core() };
        out[..f.len()].copy_from_slice(&f);
        Some(f.len())
    }
}

/// The bring-up, then `driver::start` with the devices and the listening given: the driver, if the path is active.
fn boot(fake: &mut Fake, devices: &str, listen: bool) -> Option<driver::Driver> {
    let mut scratch = vec![0u8; driver::SCRATCH];
    let mut bring = Bring::new();
    let addr = bring.up(fake, &mut scratch).expect("the bring-up");
    assert_eq!(addr, Addr(ADDRESS));
    let mut stack = Stack::new();
    stack.devices.load(devices, |_, l| panic!("{}", l));
    // The quiet path's two commands belong to the bring-up: every controller answers them, as main's did.
    fake.quiet_after_bring_up = devices.is_empty() && !listen;
    driver::start(fake, &mut bring, stack, listen, 64).expect("the start")
}

/// The loop run for `ms` of the harness's time: the steps taken, or None if it went idle.
fn run(fake: &mut Fake, d: &mut driver::Driver, ms: u64) -> Option<u32> {
    let end = fake.now + ms;
    let mut steps = 0;
    while fake.now < end {
        match d.step(fake, &mut |_| {}) { Next::Sleep(w) => fake.sleep(w), Next::Idle => return None }
        steps += 1;
    }
    Some(steps)
}

/// The bring-up's commands as main's bt_hci sent them: the first poll, reset, version, ROM version, `downloads`
/// fragments, reset, version, address.
fn bring_up_ops(downloads: usize) -> Vec<u16> {
    let mut v = vec![hci::RESET, hci::READ_LOCAL_VERSION, rtl::READ_ROM_VERSION];
    v.extend(std::iter::repeat(rtl::DOWNLOAD).take(downloads));
    v.extend([hci::RESET, hci::READ_LOCAL_VERSION, hci::READ_BD_ADDR]);
    v
}

#[test]
fn nothing_listed_is_mains_sequence_then_no_usb_call_at_all() {
    for (chip, downloads, name) in [(Chip::C8821, 3, "RTL8821C"), (Chip::C8852, 2, "RTL8852B")] {
        let mut fake = Fake::new(chip, Answers::Normal);
        assert!(boot(&mut fake, "", false).is_none(), "the quiet path");
        // The 8821C's image: 600 bytes of patch and 10 of configuration, three fragments; the 8852B's: 300, two.
        let mut want = bring_up_ops(downloads);
        want.extend([hci::SET_EVENT_MASK, hci::WRITE_INQUIRY_MODE]);
        assert_eq!(fake.ops(), want, "{}", name);
        let mask = &fake.commands.iter().find(|c| c.1 == hci::SET_EVENT_MASK).unwrap();
        assert_eq!(mask.2, 3 + 8);
        assert!(fake.logged(&format!("{} PATCHED", name)) && fake.logged("NO DEVICE LISTED") && fake.logged("ADDRESS 1C:79:2D:F0:6B:A6"));
        // After it, main's bt_hci and this one call usb_host no more: the caller idles. The bulk IN endpoint was never polled.
        assert!(fake.acl_polls.is_empty());
        println!("{}: {} commands, {} event polls, {} ms of the harness's time to the quiet end", name, fake.commands.len(), fake.event_polls.len(), fake.now - 2_800);
    }
}

#[test]
fn a_listed_device_runs_the_stacks_setup_and_polls_events_every_20_ms() {
    for chip in [Chip::C8821, Chip::C8852] {
        let mut fake = Fake::new(chip, Answers::Normal);
        let mut d = boot(&mut fake, "11:22:33:44:55:66 audio\n", false).expect("the active path");
        let first = fake.commands.len();
        // The first second: the setup, then a page to the device (which never answers here).
        run(&mut fake, &mut d, 1_000).unwrap();
        let setup: Vec<u16> = fake.ops()[first..].to_vec();
        println!("{:?} after the bring-up: {:04X?}", chip, setup);
        assert_eq!(setup, vec![hci::READ_LOCAL_FEATURES, hci::READ_BUFFER_SIZE, hci::SET_EVENT_MASK, hci::WRITE_SIMPLE_PAIRING_MODE, hci::WRITE_INQUIRY_MODE,
            hci::WRITE_CLASS_OF_DEVICE, hci::WRITE_LOCAL_NAME, hci::WRITE_DEFAULT_LINK_POLICY, hci::WRITE_PAGE_TIMEOUT, hci::WRITE_SCAN_ENABLE,
            hci::LE_SET_EVENT_MASK, hci::WRITE_LE_HOST_SUPPORTED, hci::LE_READ_BUFFER_SIZE, hci::CREATE_CONNECTION]);
        // The longest command packet: Write Local Name, 3 + 248 bytes (main's longest was a patch fragment's 3 + 253).
        assert_eq!(fake.commands.iter().map(|c| c.2).max(), Some(3 + 253));
        assert_eq!(fake.commands[first..].iter().map(|c| c.2).max(), Some(3 + 248));
        // Ten idle seconds with no link: events polled every 20 ms (50 a second), the bulk IN endpoint never, no command but
        // the page's retries (no answer: it is cancelled after 12 s and tried again 15 s later).
        let (polls, commands) = (fake.event_polls.len(), fake.commands.len());
        run(&mut fake, &mut d, 10_000).unwrap();
        let per_second = (fake.event_polls.len() - polls) as f64 / 10.0;
        assert!((49.0..=51.0).contains(&per_second), "{} polls a second", per_second);
        assert!(fake.acl_polls.is_empty());
        assert!(fake.commands.len() - commands <= 1);
        // Listening asked, nothing listed: a round every five minutes, events polled as before.
        let mut fake = Fake::new(chip, Answers::Normal);
        let mut d = boot(&mut fake, "", true).expect("the active path");
        run(&mut fake, &mut d, 40_000).unwrap();
        let rounds = fake.ops().iter().filter(|&&o| o == hci::INQUIRY).count();
        assert_eq!(rounds, 1);
        // The next round starts five minutes after this one ended (its inquiry unanswered: 12 s), its inquiry 10 s on.
        run(&mut fake, &mut d, 300_000).unwrap();
        assert_eq!(fake.ops().iter().filter(|&&o| o == hci::INQUIRY).count(), 2);
        assert!(!fake.ops().contains(&hci::WRITE_SCAN_ENABLE) && fake.acl_polls.is_empty());
    }
}

#[test]
fn a_controller_that_answers_nothing_or_errors_is_never_asked_again() {
    for answers in [Answers::Nothing, Answers::Errors] {
        let mut fake = Fake::new(Chip::C8821, answers);
        let mut d = boot(&mut fake, "11:22:33:44:55:66 audio\n", false).expect("the active path");
        let first = fake.commands.len();
        run(&mut fake, &mut d, 120_000).unwrap();
        let after: Vec<u16> = fake.ops()[first..].to_vec();
        // Each setup command once, none retried; without the buffer sizes the stack never pages.
        let mut sorted = after.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), after.len(), "{:?}: a command sent twice: {:04X?}", answers, after);
        assert!(!after.contains(&hci::CREATE_CONNECTION));
        if answers == Answers::Nothing {
            // Each waited for 3 s, then given up: 10 commands in about 30 s, then none.
            assert_eq!(after.len(), 10);
            let lost: Vec<&String> = fake.lines.iter().filter(|l| l.contains("NOT ANSWERED")).collect();
            assert_eq!(lost.len(), 10, "{:?}", fake.lines);
            assert!(fake.commands.last().unwrap().0 < 2_800 + 40_000);
        } else {
            assert!(fake.logged("COMMAND 1005 FAILED: STATUS 01"));
        }
        // The loop's pace stays 20 ms.
        let polls = fake.event_polls.len();
        run(&mut fake, &mut d, 1_000).unwrap();
        assert!((49..=51).contains(&(fake.event_polls.len() - polls)));
    }
    // The bring-up itself unanswered: it stops after 2 s, as main's did.
    let mut fake = Fake::new(Chip::C8821, Answers::Normal);
    fake.answers = Answers::Nothing;
    let mut bring = Bring::new();
    let mut silent = Fake { answers: Answers::Nothing, ..Fake::new(Chip::C8821, Answers::Nothing) };
    struct Mute<'a>(&'a mut Fake);
    impl Port for Mute<'_> {
        fn now(&mut self) -> u64 { self.0.now }
        fn sleep(&mut self, ms: u64) { self.0.sleep(ms) }
        fn command(&mut self, packet: &[u8]) -> Result<(), PortError> { self.0.commands.push((self.0.now, u16::from_le_bytes([packet[0], packet[1]]), packet.len())); Ok(()) }
        fn events(&mut self, _each: &mut dyn FnMut(&[u8])) -> Result<(), PortError> { self.0.event_polls.push(self.0.now); Ok(()) }
        fn acl_in(&mut self, _each: &mut dyn FnMut(&[u8])) -> Result<(), PortError> { Ok(()) }
        fn acl_out(&mut self, _packet: &[u8]) -> Result<(), PortError> { Ok(()) }
        fn say(&mut self, line: core::fmt::Arguments) { self.0.lines.push(line.to_string()) }
        fn firmware(&mut self, _which: Firmware, _out: &mut [u8]) -> Option<usize> { None }
    }
    let mut scratch = vec![0u8; driver::SCRATCH];
    let r = bring.up(&mut Mute(&mut silent), &mut scratch);
    assert_eq!(r, Err(driver::Fail::Timeout(hci::RESET)));
    assert_eq!(silent.commands.len(), 1);
    assert!(silent.now - 2_800 <= driver::COMMAND_MS + driver::COMMAND_POLL_MS);
    let _ = fake;
}

#[test]
fn a_flood_of_events_keeps_the_pace() {
    let mut fake = Fake::new(Chip::C8852, Answers::Flood);
    let mut d = boot(&mut fake, "11:22:33:44:55:66 hid\n", false).expect("the active path");
    let polls = fake.event_polls.len();
    let steps = run(&mut fake, &mut d, 5_000).unwrap();
    assert!((245..=251).contains(&steps) && fake.event_polls.len() - polls == steps as usize);
    assert!(fake.logged("BR/EDR BUFFERS: 8 OF 1021 BYTES"));
}

#[test]
fn a_controller_gone_stops_the_loop() {
    struct Gone(Fake, u32);
    impl Port for Gone {
        fn now(&mut self) -> u64 { self.0.now }
        fn sleep(&mut self, ms: u64) { self.0.sleep(ms) }
        fn command(&mut self, packet: &[u8]) -> Result<(), PortError> { self.0.command(packet) }
        fn events(&mut self, each: &mut dyn FnMut(&[u8])) -> Result<(), PortError> { self.1 += 1; if self.1 > 100 { Err(PortError::Gone) } else { self.0.events(each) } }
        fn acl_in(&mut self, each: &mut dyn FnMut(&[u8])) -> Result<(), PortError> { self.0.acl_in(each) }
        fn acl_out(&mut self, packet: &[u8]) -> Result<(), PortError> { self.0.acl_out(packet) }
        fn say(&mut self, line: core::fmt::Arguments) { self.0.say(line) }
        fn firmware(&mut self, which: Firmware, out: &mut [u8]) -> Option<usize> { self.0.firmware(which, out) }
    }
    let mut g = Gone(Fake::new(Chip::C8821, Answers::Normal), 0);
    let mut scratch = vec![0u8; driver::SCRATCH];
    let mut bring = Bring::new();
    bring.up(&mut g, &mut scratch).unwrap();
    let mut stack = Stack::new();
    stack.devices.load("11:22:33:44:55:66 audio\n", |_, _| {});
    g.0.quiet_after_bring_up = true;
    let mut d = driver::start(&mut g, &mut bring, stack, false, 64).unwrap().unwrap();
    let mut steps = 0;
    while let Next::Sleep(w) = d.step(&mut g, &mut |_| {}) { g.sleep(w); steps += 1; assert!(steps < 1_000); }
    assert!(g.0.logged("THE CONTROLLER IS GONE: STOPPED"));
}
