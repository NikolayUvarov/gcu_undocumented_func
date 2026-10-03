#![no_std]
#![no_main]
// audio_gw: аудиошлюз в ring 3. Драйвер AC97 (DMA-кольцо из 32 буферов), прерывания приходят
// IPC-сообщениями в точку сервиса, PCM от клиентов — через их разделяемые буферы.
// Точка расширения для TTS: синтезатор речи — обычный клиент, отдающий PCM в AUDIO_PLAY.
use mind::abi::*;
use mind::dev::{Irq, Ports};
use mind::ipc::{self, Endpoint, Message};
use mind::mem::{self, Mapping};

const RECEIVED_CAP: usize = 9;
const BUFFERS: usize = 32;
const BUFFER_BYTES: usize = 4096;
// Регистры микшера (NAM) и канала PCM OUT контроллера шины (NABM).
const RESET: u16 = 0x00; const MASTER: u16 = 0x02; const PCM_OUT: u16 = 0x18; const EXT_ID: u16 = 0x28; const EXT_CTRL: u16 = 0x2A; const FRONT_RATE: u16 = 0x2C;
const BDBAR: u16 = 0x10; const CIV: u16 = 0x14; const LVI: u16 = 0x15; const SR: u16 = 0x16; const CR: u16 = 0x1B; const GLOB_CNT: u16 = 0x2C;
const SR_DCH: u16 = 0x01; const SR_CLEAR: u16 = 0x1C; const CR_RUN: u8 = 0x01; const CR_RESET: u8 = 0x02; const CR_IOCE: u8 = 0x10;

// Четверть периода синуса, амплитуда 12000.
const SINE: [i16; 17] = [0, 2341, 4592, 6667, 8485, 9978, 11087, 11769, 12000, 11769, 11087, 9978, 8485, 6667, 4592, 2341, 0];
fn sine(phase: u32) -> i16 { // phase: 0..64 на период
    let (quarter, i) = ((phase / 16) % 4, (phase % 16) as usize);
    match quarter { 0 => SINE[i], 1 => SINE[16 - i], 2 => -SINE[i], _ => -SINE[16 - i] }
}

// Регистры BAR, адресуемые смещением от базы выданного диапазона.
#[derive(Clone, Copy)]
struct Regs { ports: Ports, base: u16 }
impl Regs {
    fn open(slot: usize) -> Option<Self> { let ports = Ports(slot); ports.range().map(|(base, _)| Self { ports, base }) }
    fn in8(&self, at: u16) -> u8 { self.ports.in8(self.base + at) }
    fn in16(&self, at: u16) -> u16 { self.ports.in16(self.base + at) }
    fn out8(&self, at: u16, value: u8) { self.ports.out8(self.base + at, value) }
    fn out16(&self, at: u16, value: u16) { self.ports.out16(self.base + at, value) }
    fn out32(&self, at: u16, value: u32) { self.ports.out32(self.base + at, value) }
}

struct Ac97 { bus: Regs, ring: Mapping, physical: u32, head: usize, started: bool, interrupts: usize }

impl Ac97 {
    fn init() -> Option<Self> {
        let ring = Mapping::new(SLOT_MEM).ok()?;
        let physical = mem::dma_physical(SLOT_MEM).ok()? as u32;
        let (mixer, bus) = (Regs::open(SLOT_DEV0)?, Regs::open(SLOT_DEV1)?);
        bus.out32(GLOB_CNT, 0x2); // снять холодный сброс кодека
        mind::time::sleep(20);
        mixer.out16(RESET, 0);
        mixer.out16(MASTER, 0x0000);
        mixer.out16(PCM_OUT, 0x0808);
        if mixer.in16(EXT_ID) & 1 != 0 { mixer.out16(EXT_CTRL, mixer.in16(EXT_CTRL) | 1); mixer.out16(FRONT_RATE, AUDIO_RATE as u16); }
        let mut device = Self { bus, ring, physical, head: 0, started: false, interrupts: 0 };
        device.reset();
        Some(device)
    }
    fn reset(&mut self) {
        self.bus.out8(CR, CR_RESET);
        for _ in 0..1000 { if self.bus.in8(CR) & CR_RESET == 0 { break; } }
        // Список дескрипторов лежит в последней странице DMA-области.
        let list = BUFFERS * BUFFER_BYTES;
        for i in 0..BUFFERS {
            let entry = &mut self.ring.as_mut_slice()[list + i * 8..list + i * 8 + 8];
            entry[..4].copy_from_slice(&(self.physical + (i * BUFFER_BYTES) as u32).to_le_bytes());
            entry[4..6].copy_from_slice(&((BUFFER_BYTES / 2) as u16).to_le_bytes());
            entry[6..8].copy_from_slice(&0x8000u16.to_le_bytes()); // прерывание по завершении буфера
        }
        self.bus.out32(BDBAR, self.physical + list as u32);
        self.head = 0; self.started = false;
    }
    // Свободные буферы: всё, что не стоит в очереди между CIV и head (один держим зазором).
    fn free(&self) -> usize {
        if !self.started { return BUFFERS - 1; }
        if self.bus.in16(SR) & SR_DCH != 0 { return BUFFERS - 1; }
        let civ = self.bus.in8(CIV) as usize;
        BUFFERS - 1 - (self.head + BUFFERS - civ) % BUFFERS
    }
    // Ставит буфер в очередь и запускает/возобновляет DMA.
    fn submit(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) {
        let at = self.head * BUFFER_BYTES; let list = BUFFERS * BUFFER_BYTES + self.head * 8;
        let bytes = fill(&mut self.ring.as_mut_slice()[at..at + BUFFER_BYTES]) & !3;
        self.ring.as_mut_slice()[list + 4..list + 6].copy_from_slice(&((bytes / 2) as u16).to_le_bytes());
        self.bus.out8(LVI, self.head as u8);
        self.head = (self.head + 1) % BUFFERS;
        if !self.started { self.bus.out8(CR, CR_RUN | CR_IOCE); self.started = true; }
    }
    fn play(&mut self, pcm: &[u8]) -> usize {
        let mut done = 0;
        while done + 4 <= pcm.len() && self.free() > 0 {
            let take = (pcm.len() - done).min(BUFFER_BYTES) & !3;
            self.submit(|buffer| { buffer[..take].copy_from_slice(&pcm[done..done + take]); take });
            done += take;
        }
        done
    }
    fn tone(&mut self, hz: usize, ms: usize) -> usize {
        let frames = AUDIO_RATE * ms.min(5000) / 1000; let mut phase: u32 = 0; let step = (hz.clamp(20, 20_000) as u32) * 64 * 256 / AUDIO_RATE as u32;
        let mut done = 0;
        while done < frames && self.free() > 0 {
            let take = (frames - done).min(BUFFER_BYTES / 4);
            self.submit(|buffer| {
                for frame in buffer[..take * 4].chunks_exact_mut(4) {
                    let sample = sine(phase / 256).to_le_bytes(); phase = phase.wrapping_add(step);
                    frame[..2].copy_from_slice(&sample); frame[2..].copy_from_slice(&sample);
                }
                take * 4
            });
            done += take;
        }
        done
    }
    fn interrupt(&mut self) {
        let status = self.bus.in16(SR);
        self.bus.out16(SR, status & SR_CLEAR); // сброс флагов «запись 1»
        self.interrupts += 1;
    }
}

// Клиенты, ждущие места в кольце: сохранённый мандат ответа и нужное число свободных буферов.
const WAITERS: usize = 8;

fn release(device: &Option<Ac97>, waiters: &mut [Option<(usize, usize)>; WAITERS], all: bool) {
    let free = device.as_ref().map_or(BUFFERS - 1, |d| d.free());
    for waiter in waiters.iter_mut() {
        if let Some((slot, want)) = *waiter { if all || free >= want { let _ = ipc::reply_saved(slot, &Message::new(free, 0)); *waiter = None; } }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut device = Ac97::init();
    let irq = Irq(SLOT_IRQ);
    let mut waiters: [Option<(usize, usize)>; WAITERS] = [None; WAITERS];
    match &device {
        Some(_) => { let _ = irq.bind(Endpoint::SERVICE); mind::println!("[AUDIO] AC97 READY: {} HZ STEREO S16, {} DMA BUFFERS", AUDIO_RATE, BUFFERS); }
        None => mind::println!("[AUDIO] NO AC97 DEVICE; GATEWAY ANSWERS WITHOUT OUTPUT"),
    }
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        if request.irq.is_some() {
            if let Some(device) = device.as_mut() {
                device.interrupt();
                if device.interrupts % 64 == 1 { mind::println!("[AUDIO] IRQ COUNT {}", device.interrupts); }
            }
            let _ = irq.ack();
            release(&device, &mut waiters, false); // буферы доиграны: будим ждущих клиентов
            continue;
        }
        let (op, arg) = (request.data[0] & 0xFF, request.data[0] >> 8);
        if op == AUDIO_WAIT && request.is_call {
            let want = arg.clamp(1, BUFFERS - 1);
            let free = device.as_ref().map_or(BUFFERS - 1, |d| d.free());
            let parked = free < want && match (waiters.iter().position(Option::is_none), ipc::save_reply()) {
                (Some(index), Ok(slot)) => { waiters[index] = Some((slot, want)); true }
                (None, Ok(slot)) => { let _ = ipc::reply_saved(slot, &Message::new(ERR_NO_SLOT, 0)); true }
                _ => false,
            };
            if !parked { let _ = ipc::reply(&Message::new(free, 0)); }
            continue;
        }
        let reply = match (op, device.as_mut()) {
            (AUDIO_INFO, d) => [d.is_some() as usize, AUDIO_RATE],
            (_, None) => [ERR_NOT_FOUND, 0],
            (AUDIO_TONE, Some(d)) => [0, d.tone(arg, request.data[1])],
            (AUDIO_STOP, Some(d)) => { d.reset(); [0, 0] }
            (AUDIO_PLAY, Some(d)) => match request.cap_received.then(|| Mapping::new(RECEIVED_CAP).ok()).flatten() {
                Some(buffer) => { let len = arg.min(buffer.len()); [d.play(&buffer.as_slice()[..len]), 0] }
                None => [ERR_INVALID, 0],
            },
            _ => [ERR_INVALID, 0],
        };
        if op == AUDIO_STOP { release(&device, &mut waiters, true); }
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
        if request.is_call { let _ = ipc::reply(&Message::new(reply[0], reply[1])); }
    }
}
