#[path = "../kernel/src/realtek_probe.rs"]
mod probe;

use probe::{Access, Device, Skip};

struct Hardware {
    config: [u32; 64],
    reads: Vec<u8>,
    writes: Vec<u32>,
    mapped: Vec<u64>,
    mmio: Vec<u64>,
    raw: u32,
    map_ok: bool,
    refuse_enable: bool,
    refuse_restore: bool,
}

impl Hardware {
    fn new(id: u32) -> (Device, Self) {
        let mut config = [0; 64];
        config[0] = id;
        config[1] = 0xf900_0401;
        config[6] = 0xfce0_0004;
        (
            Device {
                id,
                granted: false,
                base: 0xfce0_0000,
                size: 4096,
                io: false,
            },
            Self {
                config,
                reads: vec![],
                writes: vec![],
                mapped: vec![],
                mmio: vec![],
                raw: if id == probe::ETHERNET_ID {
                    0x5410_0780
                } else {
                    0x1234_2001
                },
                map_ok: true,
                refuse_enable: false,
                refuse_restore: false,
            },
        )
    }

    fn pm(&mut self, state: u32) {
        self.config[1] |= 1 << 20;
        self.config[0x34 / 4] = 0x40;
        self.config[0x40 / 4] = 0x0001_0001;
        self.config[0x44 / 4] = 0x8100 | state;
    }

    fn untouched(&self) {
        assert!(self.writes.is_empty());
        assert!(self.mmio.is_empty());
    }
}

impl Access for Hardware {
    fn config(&mut self, offset: u8) -> u32 {
        assert_eq!(offset % 4, 0);
        self.reads.push(offset);
        self.config[offset as usize / 4]
    }

    fn command(&mut self, value: u16) {
        let dword = value as u32;
        self.writes.push(dword);
        assert_eq!(
            dword & 0xffff_0004,
            0,
            "no Status W1C bits or bus mastering"
        );
        if self.refuse_enable && value & 2 != 0 || self.refuse_restore && value & 2 == 0 {
            return;
        }
        let status = self.config[1] & 0xffff_0000 & !(dword & 0xf900_0000);
        self.config[1] = status | value as u32;
    }

    fn mapped_uncached(&mut self, address: u64) -> bool {
        self.mapped.push(address);
        self.map_ok
    }

    fn read_u32(&mut self, address: u64) -> u32 {
        assert_eq!(
            self.config[1] & 6,
            2,
            "MEM on, DMA off during the single read"
        );
        assert_eq!(self.mapped, [address]);
        self.mmio.push(address);
        self.raw
    }
}

#[test]
fn reads_only_the_two_identity_registers_and_restores_command_and_status() {
    for (id, offset, revision) in [(probe::ETHERNET_ID, 0x40, 0x541), (probe::WIFI_ID, 0xf0, 2)] {
        let (device, mut hardware) = Hardware::new(id);
        hardware.pm(0);
        let before = hardware.config;
        let identity = probe::identify(device, &mut hardware).unwrap();
        assert_eq!(identity.raw, hardware.raw);
        assert_eq!(identity.revision, revision);
        assert_eq!(hardware.mmio, [device.base + offset]);
        assert_eq!(hardware.writes, [0x0403, 0x0401]);
        assert_eq!(hardware.config, before);
        assert!(hardware.reads.len() < 16);
    }
}

#[test]
fn does_not_write_when_memory_decoding_was_already_enabled() {
    let (device, mut hardware) = Hardware::new(probe::WIFI_ID);
    hardware.config[1] |= 2;
    let before = hardware.config;
    assert!(probe::identify(device, &mut hardware).is_ok());
    assert!(hardware.writes.is_empty());
    assert_eq!(hardware.config, before);
    assert_eq!(hardware.mmio.len(), 1);
}

#[test]
fn unavailable_or_undecodable_registers_restore_state_after_the_single_read() {
    for (id, values) in [
        (probe::ETHERNET_ID, [0, u32::MAX, 0x0000_0780]),
        (probe::WIFI_ID, [0, u32::MAX, 0]),
    ] {
        for raw in values {
            let (device, mut hardware) = Hardware::new(id);
            hardware.raw = raw;
            let before = hardware.config;
            assert_eq!(
                probe::identify(device, &mut hardware),
                Err(Skip::RegisterUnavailable(raw))
            );
            assert_eq!(hardware.mmio.len(), 1);
            assert_eq!(hardware.config, before);
            assert_eq!(hardware.writes, [0x0403, 0x0401]);
        }
    }
}

#[test]
fn refusal_to_enable_or_restore_is_reported_without_unbounded_retry() {
    let (device, mut hardware) = Hardware::new(probe::WIFI_ID);
    hardware.refuse_enable = true;
    let before = hardware.config;
    assert_eq!(
        probe::identify(device, &mut hardware),
        Err(Skip::DecodeEnable)
    );
    assert!(hardware.mmio.is_empty());
    assert_eq!(hardware.writes, [0x0403, 0x0401]);
    assert_eq!(hardware.config, before);
    let (device, mut hardware) = Hardware::new(probe::WIFI_ID);
    hardware.refuse_restore = true;
    assert_eq!(
        probe::identify(device, &mut hardware),
        Err(Skip::DecodeRestore(Some(0x1234_2001)))
    );
    assert_eq!(hardware.mmio.len(), 1);
    assert_eq!(hardware.writes.len(), 2);
    assert!(Skip::DecodeRestore(None).reason().contains("restoration"));
}

#[test]
fn refuses_other_owned_mastering_absent_and_non_endpoint_devices_without_changes() {
    for case in 0..6 {
        let (mut device, mut hardware) = Hardware::new(probe::ETHERNET_ID);
        let expected = match case {
            0 => {
                device.id = 0x8139_10ec;
                Skip::UnknownDevice
            }
            1 => {
                device.granted = true;
                Skip::Granted
            }
            2 => {
                hardware.config[1] |= 4;
                Skip::Mastering
            }
            3 => {
                hardware.config[0] = u32::MAX;
                Skip::ConfigUnavailable
            }
            4 => {
                hardware.config[1] = u32::MAX;
                Skip::ConfigUnavailable
            }
            _ => {
                hardware.config[3] |= 1 << 16;
                Skip::Header
            }
        };
        let before = hardware.config;
        assert_eq!(probe::identify(device, &mut hardware), Err(expected));
        assert_eq!(hardware.config, before);
        hardware.untouched();
        if case < 2 {
            assert!(hardware.reads.is_empty());
        }
    }
}

#[test]
fn d1_d2_and_d3_and_malformed_capability_chains_are_refused() {
    for state in 1..=3 {
        let (device, mut hardware) = Hardware::new(probe::WIFI_ID);
        hardware.pm(state);
        assert_eq!(
            probe::identify(device, &mut hardware),
            Err(Skip::PowerState)
        );
        hardware.untouched();
    }
    for case in 0..8 {
        let (device, mut hardware) = Hardware::new(probe::WIFI_ID);
        hardware.pm(0);
        match case {
            0 => hardware.config[0x34 / 4] = 0x20,
            1 => hardware.config[0x34 / 4] = 0x41,
            2 => hardware.config[0x40 / 4] = 0x0000_4005,
            3 => hardware.config[0x40 / 4] = u32::MAX,
            4 => {
                hardware.config[0x34 / 4] = 0xfc;
                hardware.config[0xfc / 4] = 1;
            }
            5 => hardware.config[0x44 / 4] = u32::MAX,
            6 => {
                hardware.config[0x40 / 4] |= 0x4800;
                hardware.config[0x48 / 4] = 1;
            }
            _ => hardware.config[0x34 / 4] = 0,
        }
        assert_eq!(
            probe::identify(device, &mut hardware),
            Err(Skip::Capabilities)
        );
        assert!(hardware.reads.len() < 60);
        hardware.untouched();
    }
}

#[test]
fn validates_bar_range_alignment_current_assignment_and_mapping_before_writes() {
    for case in 0..12 {
        let (mut device, mut hardware) = Hardware::new(probe::WIFI_ID);
        match case {
            0 => device.io = true,
            1 => device.base = 0,
            2 => device.base += 4,
            3 => device.size = 0,
            4 => device.size = 128,
            5 => device.size = 4095,
            6 => device.base = 1 << 32,
            7 => device.base = u64::MAX - 4095,
            8 => hardware.config[6] += 0x1000,
            9 => hardware.config[6] |= 1,
            10 => hardware.config[7] = 1,
            _ => hardware.map_ok = false,
        }
        assert_eq!(
            probe::identify(device, &mut hardware),
            Err(if case == 11 { Skip::Mapping } else { Skip::Bar })
        );
        hardware.untouched();
    }
}

#[test]
fn accepts_the_smallest_valid_bar_and_the_last_low_identity_page() {
    let (mut device, mut hardware) = Hardware::new(probe::WIFI_ID);
    device.base = 0xffff_f000;
    device.size = 256;
    hardware.config[6] = device.base as u32;
    assert_eq!(probe::identify(device, &mut hardware).unwrap().revision, 2);
    assert_eq!(hardware.mmio, [0xffff_f0f0]);
}
