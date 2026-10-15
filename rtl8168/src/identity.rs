// Register facts and primary sources are recorded in hwdocs/rtl8168.md.
pub const PCI_ID: u32 = 0x8168_10ec;
pub const TX_CONFIG_OFFSET: usize = 0x40;
pub const IDENTITY_BYTES: usize = TX_CONFIG_OFFSET + 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    WrongDevice,
    Truncated,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identity {
    pub tx_config: u32,
    pub xid: u16,
}

impl Identity {
    // Decoding a revision identifies evidence, not a supported initialization sequence.
    pub fn decode(pci_id: u32, tx_config: u32) -> Result<Self, Error> {
        if pci_id != PCI_ID {
            return Err(Error::WrongDevice);
        }
        let xid = ((tx_config >> 20) & 0xfcf) as u16;
        if tx_config == 0 || tx_config == u32::MAX || xid == 0 {
            return Err(Error::Unavailable);
        }
        Ok(Self { tx_config, xid })
    }

    // Accept an already collected register snapshot; this function does no device access.
    pub fn from_snapshot(pci_id: u32, bytes: &[u8]) -> Result<Self, Error> {
        if pci_id != PCI_ID {
            return Err(Error::WrongDevice);
        }
        let register = bytes
            .get(TX_CONFIG_OFFSET..IDENTITY_BYTES)
            .ok_or(Error::Truncated)?;
        Self::decode(pci_id, u32::from_le_bytes(register.try_into().unwrap()))
    }
}
