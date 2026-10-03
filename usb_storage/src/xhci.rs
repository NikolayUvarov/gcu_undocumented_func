// Минимальный xHCI: кольца команд/событий/передач, сброс порта, адресация и настройка конечных точек.
use mind::dev::{Dma, Mmio};

pub const TRB: usize = 16;
const RING_TRBS: usize = 256;
// Раскладка DMA-области драйвера (выровнена на 64 КиБ).
const DCBAA: usize = 0x0000; const SCRATCH_ARRAY: usize = 0x0800; const COMMAND_RING: usize = 0x1000; const EVENT_RING: usize = 0x2000;
const ERST: usize = 0x3000; const INPUT: usize = 0x4000; const OUTPUT: usize = 0x5000;
pub const EP0_RING: usize = 0x6000; pub const OUT_RING: usize = 0x7000; pub const IN_RING: usize = 0x8000;
pub const SMALL: usize = 0x9000; const SCRATCH_PAGES: usize = 0x10000; pub const DATA: usize = 0x20000;
const MAX_SCRATCH: usize = 16;

pub const TYPE_NORMAL: u32 = 1; const TYPE_SETUP: u32 = 2; const TYPE_DATA: u32 = 3; const TYPE_STATUS: u32 = 4; const TYPE_LINK: u32 = 6;
const TYPE_ENABLE_SLOT: u32 = 9; const TYPE_ADDRESS: u32 = 11; const TYPE_CONFIGURE: u32 = 12;
const EVENT_TRANSFER: u32 = 32; const EVENT_COMMAND: u32 = 33;
pub const IOC: u32 = 1 << 5; const IDT: u32 = 1 << 6; pub const ISP: u32 = 1 << 2;
const SUCCESS: u32 = 1; const SHORT_PACKET: u32 = 13;
// Биты PORTSC, которые можно записывать обратно без побочных эффектов (без RW1C и PED).
const PORT_NEUTRAL: u32 = 0x4E00_FFE9; const PORT_PED: u32 = 1 << 1; const PORT_PR: u32 = 1 << 4; const PORT_PRC: u32 = 1 << 21;

#[derive(Clone, Copy)]
struct Ring { offset: usize, index: usize, cycle: u32 }

pub struct Xhci { mmio: Mmio, pub dma: Dma, op: usize, runtime: usize, doorbells: usize, context: usize, ports: usize,
    command: Ring, event: Ring, rings: [Ring; 3], pub slot: u32, pub port: usize, pub speed: u32 }

// Активный опрос, затем сон: команды QEMU завершает сразу, чтение диска — асинхронно.
pub fn wait(mut done: impl FnMut() -> bool) -> bool {
    for attempt in 0..3_000 { if done() { return true; } if attempt > 1_000 { mind::time::sleep(10); } else { core::hint::spin_loop(); } }
    false
}

impl Xhci {
    pub fn init(mmio: Mmio, mut dma: Dma) -> Option<Self> {
        let caplength = mmio.read8(0) as usize; let hcs1 = mmio.read32(0x04); let hcs2 = mmio.read32(0x08); let hcc1 = mmio.read32(0x10);
        let (doorbells, runtime) = ((mmio.read32(0x14) & !3) as usize, (mmio.read32(0x18) & !0x1F) as usize);
        let context = if hcc1 & 4 != 0 { 64 } else { 32 };
        // Забрать контроллер у прошивки (USB Legacy Support), затем остановить и сбросить его.
        let mut cap = ((hcc1 >> 16) << 2) as usize;
        while cap != 0 {
            let value = mmio.read32(cap);
            if value & 0xFF == 1 { mmio.write32(cap, value | 1 << 24); wait(|| mmio.read32(cap) & 1 << 16 == 0); mmio.write32(cap + 4, 0); }
            let next = ((value >> 8) & 0xFF) as usize; cap = if next == 0 { 0 } else { cap + next * 4 };
        }
        let op = caplength;
        mmio.write32(op, mmio.read32(op) & !1);
        if !wait(|| mmio.read32(op + 4) & 1 != 0) { return None; }
        mmio.write32(op, 2);
        if !wait(|| mmio.read32(op) & 2 == 0 && mmio.read32(op + 4) & 1 << 11 == 0) { return None; }
        let scratch = ((hcs2 >> 21 & 0x1F) << 5 | hcs2 >> 27) as usize;
        if scratch > MAX_SCRATCH { return None; }
        dma.zero(0, DATA);
        for i in 0..scratch { let page = dma.physical(SCRATCH_PAGES + i * 4096); dma.write64(SCRATCH_ARRAY + i * 8, page); }
        if scratch > 0 { let array = dma.physical(SCRATCH_ARRAY); dma.write64(DCBAA, array); }
        mmio.write32(op + 0x38, (hcs1 & 0xFF).min(8)); // до 8 слотов: пропущенные не-накопители занимают свои
        mmio.write64(op + 0x30, dma.physical(DCBAA));
        let mut xhci = Self { mmio, dma, op, runtime, doorbells, context, ports: (hcs1 >> 24) as usize,
            command: Ring { offset: COMMAND_RING, index: 0, cycle: 1 }, event: Ring { offset: EVENT_RING, index: 0, cycle: 1 },
            rings: [Ring { offset: EP0_RING, index: 0, cycle: 1 }, Ring { offset: OUT_RING, index: 0, cycle: 1 }, Ring { offset: IN_RING, index: 0, cycle: 1 }],
            slot: 0, port: 0, speed: 0 };
        for ring in [COMMAND_RING, EP0_RING, OUT_RING, IN_RING] { xhci.link(ring); }
        xhci.mmio.write64(op + 0x18, xhci.dma.physical(COMMAND_RING) | 1);
        let event = xhci.dma.physical(EVENT_RING); xhci.dma.write64(ERST, event); xhci.dma.write32(ERST + 8, RING_TRBS as u32);
        let interrupter = runtime + 0x20;
        xhci.mmio.write32(interrupter + 0x08, 1);
        xhci.mmio.write64(interrupter + 0x18, event);
        xhci.mmio.write64(interrupter + 0x10, xhci.dma.physical(ERST));
        xhci.mmio.write32(op, 1); // RS, прерывания не используются
        if !wait(|| xhci.mmio.read32(op + 4) & 1 == 0) { return None; }
        Some(xhci)
    }

    // Последний TRB кольца — ссылка на начало с переключением бита цикла.
    fn link(&mut self, ring: usize) {
        let start = self.dma.physical(ring);
        self.dma.write64(ring + (RING_TRBS - 1) * TRB, start); self.dma.write32(ring + (RING_TRBS - 1) * TRB + 12, TYPE_LINK << 10 | 2);
    }

    fn enqueue(dma: &mut Dma, ring: &mut Ring, parameter: u64, status: u32, control: u32) -> u64 {
        let at = ring.offset + ring.index * TRB; let address = dma.physical(at);
        dma.write64(at, parameter); dma.write32(at + 8, status); dma.write32(at + 12, control | ring.cycle);
        ring.index += 1;
        if ring.index == RING_TRBS - 1 {
            let link = ring.offset + (RING_TRBS - 1) * TRB;
            let value = dma.read32(link + 12); dma.write32(link + 12, (value & !1) | ring.cycle);
            ring.index = 0; ring.cycle ^= 1;
        }
        address
    }

    // Следующее событие нужного типа (прочие, например смена состояния порта, пропускаются).
    fn event(&mut self, kind: u32) -> Option<(u64, u32, u32)> {
        let mut found = None;
        wait(|| {
            loop {
                let at = self.event.offset + self.event.index * TRB;
                let control = self.dma.read32(at + 12);
                if control & 1 != self.event.cycle { return false; }
                let parameter = self.dma.read32(at) as u64 | (self.dma.read32(at + 4) as u64) << 32; let status = self.dma.read32(at + 8);
                self.event.index += 1; if self.event.index == RING_TRBS { self.event.index = 0; self.event.cycle ^= 1; }
                let next = self.dma.physical(self.event.offset + self.event.index * TRB);
                self.mmio.write64(self.runtime + 0x20 + 0x18, next | 8);
                if (control >> 10) & 0x3F == kind { found = Some((parameter, status, control)); return true; }
            }
        });
        found
    }

    fn command(&mut self, parameter: u64, control: u32) -> Option<u32> {
        Self::enqueue(&mut self.dma, &mut self.command, parameter, 0, control);
        self.mmio.write32(self.doorbells, 0);
        let (_, status, control) = self.event(EVENT_COMMAND)?;
        (status >> 24 == SUCCESS).then_some(control >> 24)
    }

    /// Передача по кольцу `ring` (0 — EP0, 1 — bulk OUT, 2 — bulk IN) с ожиданием события; возвращает остаток.
    pub fn transfer(&mut self, ring: usize, dci: u32, trbs: &[(u64, u32, u32)]) -> Option<u32> {
        for &(parameter, status, control) in trbs { Self::enqueue(&mut self.dma, &mut self.rings[ring], parameter, status, control); }
        self.mmio.write32(self.doorbells + self.slot as usize * 4, dci);
        let (_, status, _) = self.event(EVENT_TRANSFER)?;
        matches!(status >> 24, SUCCESS | SHORT_PACKET).then_some(status & 0xFF_FFFF)
    }

    /// Стандартный запрос по EP0; данные (до 512 байт) в области SMALL.
    pub fn control(&mut self, request_type: u8, request: u8, value: u16, index: u16, length: u16) -> Option<()> {
        let setup = request_type as u64 | (request as u64) << 8 | (value as u64) << 16 | (index as u64) << 32 | (length as u64) << 48;
        let input = request_type & 0x80 != 0;
        let transfer_type = if length == 0 { 0 } else if input { 3 } else { 2 };
        let data = self.dma.physical(SMALL);
        let mut trbs = [(setup, 8, TYPE_SETUP << 10 | IDT | transfer_type << 16), (data, length as u32, TYPE_DATA << 10 | (input as u32) << 16), (0, 0, TYPE_STATUS << 10 | IOC | ((!input || length == 0) as u32) << 16)];
        if length == 0 { trbs[1] = trbs[2]; }
        self.transfer(0, 1, &trbs[..if length == 0 { 2 } else { 3 }]).map(drop)
    }

    fn portsc(&self, port: usize) -> usize { self.op + 0x400 + 0x10 * (port - 1) }

    /// Подключённый порт с включённым устройством: USB3 включается сам, USB2 сбрасывается.
    pub fn ports(&self) -> usize { self.ports }
    pub fn enable_port(&mut self, port: usize) -> bool {
        let register = self.portsc(port);
        let value = self.mmio.read32(register);
        if value & 1 == 0 { return false; }
        if value & PORT_PED == 0 {
            self.mmio.write32(register, (value & PORT_NEUTRAL) | PORT_PR);
            if !wait(|| self.mmio.read32(register) & PORT_PRC != 0) { return false; }
            let value = self.mmio.read32(register); self.mmio.write32(register, (value & PORT_NEUTRAL) | PORT_PRC);
            mind::time::sleep(20);
        }
        let value = self.mmio.read32(register);
        self.port = port; self.speed = (value >> 10) & 0xF;
        value & PORT_PED != 0
    }

    fn input_reset(&mut self) { self.dma.zero(INPUT, 0x1000); }
    fn slot_context(&mut self, entries: u32) {
        let slot = INPUT + self.context;
        self.dma.write32(slot, self.speed << 20 | entries << 27);
        self.dma.write32(slot + 4, (self.port as u32) << 16);
    }
    fn endpoint_context(&mut self, dci: u32, kind: u32, packet: u32, ring: usize) {
        let at = INPUT + self.context * (1 + dci as usize);
        let dequeue = self.dma.physical(ring) | 1;
        self.dma.write32(at + 4, 3 << 1 | kind << 3 | packet << 16);
        self.dma.write64(at + 8, dequeue);
        self.dma.write32(at + 16, if kind == 4 { 8 } else { 1024 });
    }

    /// Enable Slot + Address Device для устройства на включённом порту.
    pub fn address(&mut self) -> Option<()> {
        self.slot = self.command(0, TYPE_ENABLE_SLOT << 10)?;
        self.dma.zero(OUTPUT, 0x1000);
        let output = self.dma.physical(OUTPUT); self.dma.write64(DCBAA + self.slot as usize * 8, output);
        self.input_reset();
        self.dma.write32(INPUT + 4, 0b11);
        self.slot_context(1);
        let packet = match self.speed { 4 => 512, 3 => 64, _ => 8 };
        self.endpoint_context(1, 4, packet, EP0_RING);
        let input = self.dma.physical(INPUT);
        self.command(input, TYPE_ADDRESS << 10 | self.slot << 24).map(drop)
    }

    /// Configure Endpoint для пары bulk-точек (номер точки и максимальный размер пакета).
    pub fn configure(&mut self, out: (u32, u32), input: (u32, u32)) -> Option<(u32, u32)> {
        let (dci_out, dci_in) = (out.0 * 2, input.0 * 2 + 1);
        self.input_reset();
        self.dma.write32(INPUT + 4, 1 | 1 << dci_out | 1 << dci_in);
        self.slot_context(dci_out.max(dci_in));
        self.endpoint_context(dci_out, 2, out.1, OUT_RING);
        self.endpoint_context(dci_in, 6, input.1, IN_RING);
        let context = self.dma.physical(INPUT);
        self.command(context, TYPE_CONFIGURE << 10 | self.slot << 24)?;
        Some((dci_out, dci_in))
    }
}
