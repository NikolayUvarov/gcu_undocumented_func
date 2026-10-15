use crate::{btc_session, download, fwcmd, initial_phy, io::{self, RegisterIo}, model, phycap, post_mac, runtime_mac, rx};

// Extend one cold device with write-triggered RF/LTE/calibration replies, without reseeding firmware or MAC guards.
struct Device {
    inner: model::Device, rf: [[u32; 256]; 2], rf_read: Option<(usize, usize)>,
    lte: [u32; 2], lte_pending: Option<u32>, bt: u32, drck: bool, addck: [bool; 2],
    msbk: [[bool; 2]; 2], dadck: [[bool; 2]; 2], nctl_stuck: bool, rck_stuck: bool,
    drck_stuck: bool, fail_write: Option<usize>,
}
impl Device {
    fn new(inner: model::Device) -> Self {
        Self { inner, rf: [[0; 256]; 2], rf_read: None, lte: [0; 2], lte_pending: None, bt: 0,
            drck: false, addck: [false; 2], msbk: [[false; 2]; 2], dadck: [[false; 2]; 2],
            nctl_stuck: false, rck_stuck: false, drck_stuck: false, fail_write: None }
    }
    fn write(&mut self, at: usize, width: usize, value: u32) -> io::Result<()> {
        self.inner.now += 1;
        match width { 1 => self.inner.write8(at, value as u8)?, 2 => self.inner.write16(at, value as u16)?, _ => self.inner.write32(at, value)? }
        if at == 0x10378 { self.rf_read = Some((((value >> 8) & 7) as usize, (value & 255) as usize)); }
        if at == 0x10370 {
            let path = ((value >> 28) & 3) as usize; assert!(path < 2);
            let reg = ((value >> 20) & 255) as usize;
            let mask = if value >> 31 != 0 { self.inner.get(0x10374, 4) & 0xfffff } else { 0xfffff };
            self.rf[path][reg] = (self.rf[path][reg] & !mask) | (value & mask); self.rf_read = None;
            if reg == 0x1b && value & 0xfffff == 0x240 && !self.rck_stuck {
                self.rf[path][0x1c] |= 8; self.rf[path][0x1b] = (7 + path as u32 * 4) << 10;
            }
        }
        if at == 0xdaf0 { assert!(self.lte_pending.is_none()); self.lte_pending = Some(value); }
        if at == 0x1c0cc && value & 64 != 0 { self.drck = true; }
        for path in 0..2 {
            let base = 0x1c000 + path * 0x100;
            if at == base + 0xf4 && value & 0x300 == 0x100 { self.addck[path] = true; }
            for iq in 0..2 {
                if at == base + iq * 0x20 + 4 && value & 1 != 0 { self.msbk[path][iq] = true; }
                if at == base + iq * 0x20 + 12 && value & 4 != 0 { self.dadck[path][iq] = true; }
            }
        }
        if self.fail_write == Some(at) { self.fail_write = None; return Err(io::Fault::new(io::FaultKind::Access, at, value)); }
        Ok(())
    }
}
impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> io::Result<u8> {
        self.inner.now += 1;
        if at == 0xdaf3 {
            if let Some(command) = self.lte_pending.take() {
                if command >> 30 == 3 { self.lte[usize::from(command & 255 == 0x3c)] = self.inner.get(0xdaf4, 4); }
            }
            return Ok(0x20);
        }
        self.inner.read8(at)
    }
    fn read16(&mut self, at: usize) -> io::Result<u16> { self.inner.now += 1; self.inner.read16(at) }
    fn read32(&mut self, at: usize) -> io::Result<u32> {
        self.inner.now += 1;
        if at == 0xac { return Ok(self.bt); }
        if at == 0xdaf8 { let command = self.lte_pending.unwrap(); assert_eq!(command >> 30, 2); return Ok(self.lte[usize::from(command & 255 == 0x3c)]); }
        if at == 0x1174c { return Ok(self.rf_read.map_or(0, |(path, reg)| 1 << 26 | self.rf[path][reg])); }
        if at == 0x18080 && self.nctl_stuck { return Ok(0); }
        if at == 0x1c0d0 { return Ok(if self.drck && !self.drck_stuck { 17 << 15 | 8 } else { 0 }); }
        for path in 0..2 {
            let base = 0x1c000 + path * 0x100;
            if at == base + 0xfc { return Ok(if self.addck[path] { 0x123 << 10 | 0x155 } else { 0 }); }
            for iq in 0..2 {
                if at == base + 0x40 + iq * 0x24 { return Ok(u32::from(self.msbk[path][iq]) << 31); }
                if at == base + 0x5c + iq * 0x24 {
                    let index = (self.inner.get(base + iq * 0x20, 4) >> 1) & 15;
                    return Ok(u32::from(self.dadck[path][iq]) * 4 | (0x10 + path as u32 * 0x40 + iq as u32 * 0x20 + index) << 24);
                }
            }
        }
        self.inner.read32(at)
    }
    fn write8(&mut self, at: usize, value: u8) -> io::Result<()> { self.write(at, 1, value as u32) }
    fn write16(&mut self, at: usize, value: u16) -> io::Result<()> { self.write(at, 2, value as u32) }
    fn write32(&mut self, at: usize, value: u32) -> io::Result<()> { self.write(at, 4, value) }
    fn now_us(&mut self) -> u64 { self.inner.now }
    fn delay_us(&mut self, us: u32) -> io::Result<()> { self.inner.delay_us(us) }
}
impl fwcmd::Bus for Device { fn pci_command(&mut self) -> io::Result<u16> { Ok(self.inner.command) } }
impl btc_session::DeviceIo for Device {
    fn pci_id(&mut self) -> io::Result<u32> { Ok(0xb85210ec) }
    fn bar_len(&self) -> usize { 0x20000 }
}

fn run(speed: u8, configure: impl FnOnce(&mut Device), check: impl FnOnce(&mut Device, Result<initial_phy::Observations, initial_phy::Failure>)) {
    download::tests::with_synthetic_firmware(|verified| {
        let mut raw = model::Device::new(speed); let board = model::read_board(&mut raw);
        let mut region = model::Region::new(); let (mut queue, mut rings) = region.pair(&mut raw);
        model::prepare_receive(&mut raw, &mut queue, &mut rings).unwrap();
        let ready = download::run(&mut raw, &mut queue, verified).unwrap();
        let caps = phycap::query(&mut raw, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap();
        let runtime = runtime_mac::prepare(&mut raw, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap();
        let receiving = post_mac::activate(&mut raw, &mut queue, &mut rings, &board, runtime_mac::BAR_BYTES, &runtime).unwrap();
        let mut device = Device::new(raw); configure(&mut device); let mut scratch = vec![0; rx::BUFFER_BYTES];
        let result = initial_phy::diagnose(&mut device, &mut queue, &mut rings, &receiving, ready, &board, &caps, &mut scratch);
        check(&mut device, result);
        assert_eq!(device.inner.get(0xc348, 4) & 0xffff, 0);
        assert_eq!(device.inner.get(fwcmd::DMA_STOP, 4) & 0xb0f00, 0xb0f00);
        let stopped = rings.stop(&mut device); assert_eq!((stopped.request, stopped.hci, stopped.idle), (None, None, None));
        runtime_mac::disable_radio(&mut device).unwrap();
        assert_eq!(download::stop(&mut device, &mut queue), download::Stop { dma: None, cpu: None });
        model::power_off(&mut device.inner); assert_eq!(device.inner.command, 0);
    });
}

#[test]
fn cold_gen1_and_gen2_complete_owned_initial_phy_and_then_shut_down() {
    for speed in [1, 2] { run(speed, |_| {}, |d, result| {
        let observed = result.unwrap(); assert_eq!(observed.rck.values, [7, 11]); assert_eq!(observed.dack.drck, 17);
        assert_eq!(observed.dack.completion, [[3, 3]; 2]); assert_eq!((observed.rx_path, observed.rx_nss), (3, 2));
        assert_eq!(observed.encoded_rts, 0x2ff);
        let expected = [(2, 8, 0), (2, 9, 0), (2, 16, 2), (2, 16, 0), (2, 16, 1),
            (2, 16, 5), (2, 16, 5), (2, 16, 3), (2, 16, 10), (2, 16, 5)];
        assert_eq!(observed.submitted as usize, expected.len());
        let headers: Vec<_> = d.inner.normal_packets.iter().map(|p| (p[0] & 3, p[0] >> 2, p[1])).collect();
        assert_eq!(headers, expected);
        assert_eq!(d.inner.normal_packets.len(), observed.submitted as usize);
        for (index, packet) in d.inner.normal_packets.iter().enumerate() { assert_eq!(packet[3], index as u8); }
        assert_eq!(d.inner.get(0xce40, 4), 0x2b); assert_eq!(d.inner.get(0x9c18, 4) & 3, 1);
        assert_eq!(d.inner.get(0x8040, 1), 15); assert!(d.inner.now < 2_000_000);
    }); }
}

#[test]
fn failed_radio_ack_prevents_btc_and_all_initial_calibration() {
    run(2, |d| d.inner.normal_ack = model::NormalAck::Missing, |d, result| {
        let error = result.unwrap_err(); assert!(matches!(error.cause, initial_phy::Cause::Phy(_)));
        assert_eq!(d.inner.normal_packets.len(), 1); assert!(!d.drck && d.addck == [false; 2]);
        assert!(!d.inner.writes.iter().any(|&(at, _, value)| at == 0xce40 && value == 0x2b));
    });
}

#[test]
fn bt_busy_denies_initialization_before_nctl_and_rfk() {
    run(1, |d| d.bt = 2 | 32, |d, result| {
        assert!(matches!(result.unwrap_err().cause, initial_phy::Cause::Coexistence(_)));
        assert!(!d.drck); assert!(!d.inner.writes.iter().any(|&(at, _, value)| at == 0x18080 && value == 4));
    });
}

#[test]
fn failed_nctl_rck_and_dack_cannot_publish_path_or_report_readiness() {
    for failed in 0..3 { run(2, |d| match failed { 0 => d.nctl_stuck = true, 1 => d.rck_stuck = true, _ => d.drck_stuck = true }, |d, result| {
        let error = result.unwrap_err();
        match failed {
            0 | 1 => assert!(matches!(error.cause, initial_phy::Cause::Phy(_))),
            _ => assert!(matches!(error.cause, initial_phy::Cause::Dack(_))),
        }
        assert!(!d.inner.writes.iter().any(|&(at, _, value)| at == 0xce40 && value == 0x2b));
        assert!(d.inner.now < 2_000_000);
    }); }
}

#[test]
fn final_report_write_failure_retains_independent_cleanup_evidence() {
    run(1, |d| d.fail_write = Some(0xce40), |_, result| {
        let error = result.unwrap_err();
        let initial_phy::Cause::Reports { stage, report_stop, receive_stop, radio_stop, command_stop, .. } = error.cause else { panic!("{error:?}") };
        assert_eq!(stage, "ppdu"); assert!(report_stop.is_none() && radio_stop.is_none());
        assert!(receive_stop.is_some() && command_stop.is_some());
    });
}
