//! Доступ драйверов ring 3 к устройствам строго через мандаты: порты, линии IRQ, ввод, кадр.
use crate::abi::*;
use crate::ipc::Endpoint;
use crate::sys::{call, check, syscall, Result};

/// Что лежит в слоте мандата: (вид CAP_KIND_*, база, размер).
pub fn cap_info(slot: usize) -> (usize, usize, usize) { let raw = syscall(SYSCALL_CAP_INFO, slot, 0, [0; 4]); (raw.result, raw.arg2, raw.msg[2]) }

/// Диапазон портов ввода-вывода в слоте мандата (номера портов абсолютные).
#[derive(Clone, Copy)]
pub struct Ports(pub usize);

impl Ports {
    /// База и число портов выданного диапазона (для устройств PCI с BAR).
    pub fn range(&self) -> Option<(u16, u16)> {
        match cap_info(self.0) { (CAP_KIND_PORTS, base, count) => Some((base as u16, count as u16)), _ => None }
    }
    fn read(&self, port: u16, width: usize) -> usize { syscall(SYSCALL_PORT_IN, self.0, port as usize, [0, width, 0, 0]).result }
    fn write(&self, port: u16, width: usize, value: usize) { syscall(SYSCALL_PORT_OUT, self.0, port as usize, [value, width, 0, 0]); }
    pub fn in8(&self, port: u16) -> u8 { self.read(port, 1) as u8 }
    pub fn in16(&self, port: u16) -> u16 { self.read(port, 2) as u16 }
    pub fn in32(&self, port: u16) -> u32 { self.read(port, 4) as u32 }
    pub fn out8(&self, port: u16, value: u8) { self.write(port, 1, value as usize) }
    pub fn out16(&self, port: u16, value: u16) { self.write(port, 2, value as usize) }
    pub fn out32(&self, port: u16, value: u32) { self.write(port, 4, value as usize) }
    /// Пакетное чтение 16-битных слов (данные ATA) прямо в буфер.
    pub fn read_words(&self, port: u16, buffer: &mut [u16]) -> Result<usize> {
        check(syscall(SYSCALL_PORT_IN_BLOCK, self.0, port as usize, [0, 0, buffer.as_mut_ptr() as usize, buffer.len()]).result)
    }
}

/// Линия прерывания в слоте мандата. После срабатывания ядро маскирует её до wait/ack.
#[derive(Clone, Copy)]
pub struct Irq(pub usize);

impl Irq {
    /// Открывает линию и спит до следующего прерывания.
    pub fn wait(&self) -> Result<()> { check(call(SYSCALL_IRQ_WAIT, self.0, 0)).map(drop) }
    /// Доставлять прерывания сообщениями в точку IPC (флаг `irq` в `Received`).
    pub fn bind(&self, endpoint: Endpoint) -> Result<()> { check(call(SYSCALL_IRQ_BIND, self.0, endpoint.0)).map(drop) }
    /// Подтверждает обработку и снова открывает линию.
    pub fn ack(&self) -> Result<()> { check(call(SYSCALL_IRQ_ACK, self.0, 0)).map(drop) }
}

/// Событие клавиатуры от драйвера с мандатом ввода.
pub fn input_event(app: u8, shell: u8, background: bool) -> Result<()> {
    check(syscall(SYSCALL_INPUT_EVENT, app as usize, shell as usize, [background as usize, 0, 0, 0]).result).map(drop)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frame { Unchanged, Dirty, NewSource }

/// Композитор: состояние активного экрана; при NewSource мандат на новый экран лежит в `slot`.
pub fn compositor_pull(slot: usize) -> Result<Frame> {
    check(call(SYSCALL_COMPOSITOR_PULL, slot, 0)).map(|state| match state { 0 => Frame::Unchanged, 1 => Frame::Dirty, _ => Frame::NewSource })
}
