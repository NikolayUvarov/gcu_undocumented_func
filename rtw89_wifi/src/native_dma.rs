//! Three disjoint retained DMA regions; installation never enables packet transmission.

use crate::{fwcmd, io::Fault, rx, tx};

pub const ALLOCATION_BYTES: usize = 8 * 1024 * 1024;
pub const COMMAND_BYTES: usize = 128 * 1024;
pub const RECEIVE_BYTES: usize = rx::DMA_BYTES;
pub const TRANSMIT_OFFSET: usize = COMMAND_BYTES + RECEIVE_BYTES;
pub const TRANSMIT_BYTES: usize = ALLOCATION_BYTES - TRANSMIT_OFFSET;

pub struct TransmitError(tx::Error);

impl core::fmt::Debug for TransmitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { self.0.fmt(f) }
}

#[derive(Debug)]
pub enum Error { Region, Command(fwcmd::Error), Receive(rx::Error), Transmit(TransmitError) }

#[derive(Debug)]
pub struct TransmitStop { pub request: Option<Fault>, pub idle: Option<Fault> }

pub struct Queues<'a> {
    pub command: fwcmd::Queue<'a>,
    pub receive: rx::Rings<'a>,
    pub(crate) transmit: tx::Queue<'a>,
}

impl<'a> Queues<'a> {
    /// # Safety
    /// The caller exclusively owns the full coherent x86 WB allocation, with UC MMIO for the same PCI device.
    /// No old device transaction may access it; retain it after every result/drop until verified PCI quiescence.
    /// No later CPU alias may access any of these regions while their queue objects exist.
    pub unsafe fn new(bus: &mut impl fwcmd::Bus, address: *mut u8, length: usize, physical: u64) -> Result<Self, Error> {
        if address.is_null() || address as usize & 4095 != 0 || physical == 0 || physical & 4095 != 0
            || length < ALLOCATION_BYTES || length > isize::MAX as usize
            || (address as usize).checked_add(length).is_none()
            || physical.checked_add(length as u64).is_none_or(|end| end > 1u64 << 32)
            || fwcmd::DMA_BYTES > COMMAND_BYTES || tx::DMA_BYTES > TRANSMIT_BYTES
            || RECEIVE_BYTES & 4095 != 0 || TRANSMIT_OFFSET & 4095 != 0 {
            return Err(Error::Region);
        }
        let command = fwcmd::Queue::new(bus, address, COMMAND_BYTES, physical).map_err(Error::Command)?;
        let receive = rx::Rings::new(bus, address.add(COMMAND_BYTES), RECEIVE_BYTES, physical + COMMAND_BYTES as u64)
            .map_err(Error::Receive)?;
        let transmit = tx::Queue::new(bus, address.add(TRANSMIT_OFFSET), TRANSMIT_BYTES, physical + TRANSMIT_OFFSET as u64)
            .map_err(|error| Error::Transmit(TransmitError(error)))?;
        Ok(Self { command, receive, transmit })
    }

    /// Requires the completed source PCIe preparation, MEM-only PCI and stopped HCI/packet queues.
    pub fn install_transmit(&mut self, bus: &mut impl fwcmd::Bus) -> Result<(), Error> {
        self.transmit.install(bus).map_err(|error| Error::Transmit(TransmitError(error)))
    }

    /// Retains every region; this result alone never establishes PCI quiescence.
    pub fn stop_transmit(&mut self, bus: &mut impl crate::io::RegisterIo) -> TransmitStop {
        let stop = self.transmit.stop(bus);
        TransmitStop { request: stop.request, idle: stop.idle }
    }
}
