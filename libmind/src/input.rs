use crate::abi::SYSCALL_READ_KEY;
use crate::sys::call;

/// Следующий код клавиши активной программы (скан-код PS/2 или байт UART).
pub fn read_key() -> Option<u8> { match call(SYSCALL_READ_KEY, 0, 0) as u8 { 0 => None, key => Some(key) } }

/// Esc в обоих путях ввода: скан-код 0x01 и байт 0x1B.
pub fn is_escape(key: u8) -> bool { key == 0x01 || key == 0x1B }

/// Разбирает накопленный ввод, завершает процесс по Esc, затем спит `ms`. Возвращает последнюю клавишу.
pub fn wait_or_exit(ms: usize) -> Option<u8> {
    let mut last = None;
    while let Some(key) = read_key() {
        if is_escape(key) { crate::process::exit(); }
        last = Some(key);
    }
    crate::time::sleep(ms);
    last
}
