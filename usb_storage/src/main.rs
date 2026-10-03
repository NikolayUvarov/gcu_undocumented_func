#![no_std]
#![no_main]
// Драйвер USB mass storage в ring 3: контроллер xHCI по мандату MMIO, Bulk-Only Transport, SCSI READ(10).
mod xhci;

use mind::abi::{BootInfo, EP_BLOCK_USB, SLOT_DEV0, SLOT_MEM};
use mind::block::{self, Driver};
use mind::dev::{Dma, Mmio};
use xhci::{Xhci, DATA, IOC, ISP, SMALL, TYPE_NORMAL};

const CBW: usize = SMALL + 0x800; const CSW: usize = SMALL + 0xC00;

// Состояние Bulk-Only Transport: номера контекстов bulk-точек и счётчик тегов.
struct Bot { dci_out: u32, dci_in: u32, tag: u32 }

struct Storage { host: Xhci, bot: Bot, sectors: u64 }

// Один цикл BOT: CBW по bulk OUT, данные по bulk IN (если есть), CSW по bulk IN. Возвращает принятые байты.
fn scsi(host: &mut Xhci, bot: &mut Bot, command: &[u8], length: usize) -> Option<usize> {
    bot.tag = bot.tag.wrapping_add(1);
    let cbw = host.dma.bytes(CBW, 31);
    cbw.fill(0);
    cbw[..4].copy_from_slice(b"USBC"); cbw[4..8].copy_from_slice(&bot.tag.to_le_bytes()); cbw[8..12].copy_from_slice(&(length as u32).to_le_bytes());
    cbw[12] = if length > 0 { 0x80 } else { 0 }; cbw[14] = command.len() as u8; cbw[15..15 + command.len()].copy_from_slice(command);
    let (cbw_address, data, csw) = (host.dma.physical(CBW), host.dma.physical(DATA), host.dma.physical(CSW));
    host.transfer(1, bot.dci_out, &[(cbw_address, 31, TYPE_NORMAL << 10 | IOC)])?;
    let residue = if length > 0 { host.transfer(2, bot.dci_in, &[(data, length as u32, TYPE_NORMAL << 10 | IOC | ISP)])? as usize } else { 0 };
    host.transfer(2, bot.dci_in, &[(csw, 13, TYPE_NORMAL << 10 | IOC | ISP)])?;
    let status = host.dma.bytes(CSW, 13);
    (&status[..4] == b"USBS" && status[4..8] == bot.tag.to_le_bytes() && status[12] == 0).then_some(length - residue)
}

// Дескрипторы конфигурации: интерфейс класса 08/06/50 и его bulk-точки; затем SET_CONFIGURATION и Configure Endpoint.
fn attach(host: &mut Xhci) -> Option<Bot> {
    host.address()?;
    host.control(0x80, 6, 0x0200, 0, 9)?;
    let header = host.dma.bytes(SMALL, 4);
    let total = u16::from_le_bytes([header[2], header[3]]).min(512);
    host.control(0x80, 6, 0x0200, 0, total)?;
    let config = host.dma.bytes(SMALL, total as usize);
    let value = config[5];
    let (mut at, mut storage, mut out, mut input) = (0usize, false, None, None);
    while at + 2 <= config.len() && config[at] >= 2 {
        let (length, kind) = (config[at] as usize, config[at + 1]);
        if kind == 4 && at + 8 <= config.len() { storage = config[at + 5] == 8 && config[at + 6] == 6 && config[at + 7] == 0x50; }
        if kind == 5 && storage && at + 6 <= config.len() && config[at + 3] & 3 == 2 {
            let endpoint = ((config[at + 2] & 0x0F) as u32, u16::from_le_bytes([config[at + 4], config[at + 5]]) as u32 & 0x7FF);
            if config[at + 2] & 0x80 != 0 { input.get_or_insert(endpoint); } else { out.get_or_insert(endpoint); }
        }
        at += length;
    }
    let (out, input) = (out?, input?);
    host.control(0x00, 9, value as u16, 0, 0)?;
    let (dci_out, dci_in) = host.configure(out, input)?;
    Some(Bot { dci_out, dci_in, tag: 0 })
}

// TEST UNIT READY (с REQUEST SENSE после «unit attention»), затем READ CAPACITY(10); нужны секторы по 512 байт.
fn capacity(host: &mut Xhci, bot: &mut Bot) -> Option<u64> {
    for _ in 0..5 {
        if scsi(host, bot, &[0x00, 0, 0, 0, 0, 0], 0).is_some() { break; }
        let _ = scsi(host, bot, &[0x03, 0, 0, 0, 18, 0], 18);
        mind::time::sleep(50);
    }
    scsi(host, bot, &[0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0], 8)?;
    let reply = host.dma.bytes(DATA, 8);
    let last = u32::from_be_bytes(reply[..4].try_into().unwrap()) as u64; let size = u32::from_be_bytes(reply[4..8].try_into().unwrap());
    (size == 512).then_some(last + 1)
}

impl Storage {
    // Первый порт с устройством класса mass storage; остальные устройства получают адрес и пропускаются.
    fn probe() -> Option<Self> {
        let mut host = Xhci::init(Mmio::map(SLOT_DEV0).ok()?, Dma::map(SLOT_MEM).ok()?)?;
        for port in 1..=host.ports() {
            if !host.enable_port(port) { continue; }
            let Some(mut bot) = attach(&mut host) else { continue };
            let Some(sectors) = capacity(&mut host, &mut bot) else { continue };
            return Some(Self { host, bot, sectors });
        }
        None
    }
}

impl Driver for Storage {
    fn sectors(&self) -> u64 { self.sectors }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool {
        let lba = (lba as u32).to_be_bytes(); let blocks = (count as u16).to_be_bytes();
        let command = [0x28, 0, lba[0], lba[1], lba[2], lba[3], 0, blocks[0], blocks[1], 0];
        match scsi(&mut self.host, &mut self.bot, &command, count * 512) {
            Some(got) if got == count * 512 => { out[..got].copy_from_slice(self.host.dma.bytes(DATA, got)); true }
            _ => false,
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut storage = Storage::probe();
    match &storage {
        Some(s) => mind::println!("[USB] MASS STORAGE ON PORT {} (SPEED {}): {} SECTORS", s.host.port, s.host.speed, s.sectors),
        None => mind::println!("[USB] NO MASS STORAGE DEVICE"),
    }
    block::serve(EP_BLOCK_USB, storage.as_mut().map(|s| s as &mut dyn Driver));
}
