#[allow(dead_code)]
#[path = "../../rtl8168/src/identity.rs"]
mod wired;
#[allow(dead_code)]
#[path = "../../rtw89_wifi/src/registers.rs"]
mod wireless;

pub const WIFI_ID: u32 = (wireless::DEVICE_ID as u32) << 16 | wireless::VENDOR_ID as u32;
pub const ETHERNET_ID: u32 = wired::PCI_ID;

pub trait Access {
    fn config(&mut self, offset: u8) -> u32;
    fn command(&mut self, value: u16);
    fn mapped_uncached(&mut self, address: u64) -> bool;
    fn read_u32(&mut self, address: u64) -> u32;
}

#[derive(Clone, Copy)]
pub struct Device {
    pub id: u32,
    pub granted: bool,
    pub base: u64,
    pub size: u64,
    pub io: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identity {
    pub raw: u32,
    pub revision: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Skip {
    UnknownDevice,
    Granted,
    ConfigUnavailable,
    Header,
    Mastering,
    Capabilities,
    PowerState,
    Bar,
    Mapping,
    DecodeEnable,
    DecodeRestore(Option<u32>),
    RegisterUnavailable(u32),
}

impl Skip {
    pub fn reason(self) -> &'static str {
        match self {
            Self::UnknownDevice => "device ID not supported by this probe",
            Self::Granted => "already granted to a driver",
            Self::ConfigUnavailable => "PCI configuration unavailable or changed",
            Self::Header => "not an ordinary PCI endpoint",
            Self::Mastering => "bus mastering is still enabled",
            Self::Capabilities => "malformed or inaccessible PCI capability list",
            Self::PowerState => "power-management state is not D0",
            Self::Bar => "BAR2 is invalid, changed, misaligned or outside the low identity map",
            Self::Mapping => "register lacks an existing uncached identity mapping",
            Self::DecodeEnable => "temporary memory decoding did not take effect",
            Self::DecodeRestore(_) => {
                "original PCI command could not be verified after restoration"
            }
            Self::RegisterUnavailable(_) => {
                "identity register is zero, all-ones or has no usable revision"
            }
        }
    }
}

// All exits after enabling MEM converge on restoration; no DMA, PM or MMIO writes occur.
pub fn identify(device: Device, access: &mut impl Access) -> Result<Identity, Skip> {
    let offset = match device.id {
        ETHERNET_ID => wired::TX_CONFIG_OFFSET as u64,
        WIFI_ID => wireless::SYS_CFG1 as u64,
        _ => return Err(Skip::UnknownDevice),
    };
    if device.granted {
        return Err(Skip::Granted);
    }
    if access.config(0) != device.id {
        return Err(Skip::ConfigUnavailable);
    }
    if access.config(0x0c) >> 16 & 0x7f != 0 {
        return Err(Skip::Header);
    }
    let command_status = access.config(4);
    if command_status == u32::MAX {
        return Err(Skip::ConfigUnavailable);
    }
    let original = command_status as u16;
    if original & 4 != 0 {
        return Err(Skip::Mastering);
    }
    powered(command_status, access)?;
    let address = register_address(device, offset, access)?;
    if !access.mapped_uncached(address) {
        return Err(Skip::Mapping);
    }

    let changed = original & 2 == 0;
    if changed {
        access.command(original | 2);
    }
    let mut observed = None;
    let result = (|| {
        if changed && access.config(4) as u16 != (original | 2) {
            return Err(Skip::DecodeEnable);
        }
        let raw = access.read_u32(address);
        observed = Some(raw);
        if raw == 0 || raw == u32::MAX {
            return Err(Skip::RegisterUnavailable(raw));
        }
        let revision = if device.id == ETHERNET_ID {
            wired::Identity::decode(device.id, raw)
                .map_err(|_| Skip::RegisterUnavailable(raw))?
                .xid
        } else {
            wireless::chip_cut(raw).ok_or(Skip::RegisterUnavailable(raw))? as u16
        };
        Ok(Identity { raw, revision })
    })();
    if changed {
        access.command(original);
        if access.config(4) as u16 != original {
            return Err(Skip::DecodeRestore(observed));
        }
    }
    result
}

fn powered(command_status: u32, access: &mut impl Access) -> Result<(), Skip> {
    if command_status & (1 << 20) == 0 {
        return Ok(());
    }
    let pointer = access.config(0x34);
    if pointer == u32::MAX {
        return Err(Skip::Capabilities);
    }
    let mut at = pointer as u8;
    if at == 0 {
        return Err(Skip::Capabilities);
    }
    let mut seen = 0u64;
    let mut pm_seen = false;
    for _ in 0..48 {
        if at == 0 {
            return Ok(());
        }
        if at < 0x40 || at & 3 != 0 || seen & (1u64 << (at / 4)) != 0 {
            return Err(Skip::Capabilities);
        }
        seen |= 1u64 << (at / 4);
        let header = access.config(at);
        if header == u32::MAX || header as u8 == 0 {
            return Err(Skip::Capabilities);
        }
        if header as u8 == 1 {
            if pm_seen || at > 0xf8 {
                return Err(Skip::Capabilities);
            }
            pm_seen = true;
            let pmcsr = access.config(at + 4);
            if pmcsr == u32::MAX {
                return Err(Skip::Capabilities);
            }
            if pmcsr & 3 != 0 {
                return Err(Skip::PowerState);
            }
        }
        at = (header >> 8) as u8;
    }
    if at == 0 {
        Ok(())
    } else {
        Err(Skip::Capabilities)
    }
}

fn register_address(device: Device, offset: u64, access: &mut impl Access) -> Result<u64, Skip> {
    if device.io
        || device.base == 0
        || !device.size.is_power_of_two()
        || device.base % device.size != 0
        || device.base & 15 != 0
        || offset.checked_add(4).is_none_or(|end| end > device.size)
        || device
            .base
            .checked_add(device.size)
            .is_none_or(|end| end > 1u64 << 32)
    {
        return Err(Skip::Bar);
    }
    let raw = access.config(0x18);
    if raw == u32::MAX || raw & 1 != 0 || !matches!(raw & 6, 0 | 4) {
        return Err(Skip::Bar);
    }
    let high = if raw & 6 == 4 {
        access.config(0x1c) as u64
    } else {
        0
    };
    let current = (raw & !15) as u64 | high << 32;
    if current != device.base {
        return Err(Skip::Bar);
    }
    device.base.checked_add(offset).ok_or(Skip::Bar)
}
