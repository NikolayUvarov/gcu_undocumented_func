#![no_std]
#![no_main]
// loader: читает ELF приложений с диска через vfs_server и запускает их (SPAWN_IMAGE)
// по запросу шелла ядра или программ; ядро образов приложений не хранит.
use core::fmt::Write;
use mind::abi::*;
use mind::fs::{self, File};
use mind::ipc::{self, Endpoint, Message};
use mind::mem::{Mapping, Pages};
use mind::sys::Error;
use mind::util::FixedBuf;

const RECEIVED_CAP: usize = 9;
const MAX_IMAGE: usize = 4 * 1024 * 1024;

// Имя программы -> файл: «clock» -> clock.elf; путь с точкой или каталогом используется как есть.
fn path_for(name: &[u8], path: &mut FixedBuf<64>) -> Result<(), Error> {
    let text = core::str::from_utf8(name).map_err(|_| Error::Invalid)?;
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_graphic()) { return Err(Error::Invalid); }
    let _ = if text.contains('.') || text.contains('/') { write!(path, "{}", text) } else { write!(path, "{}.elf", text) };
    Ok(())
}

// Имя задачи для ps: файл без каталога и расширения .elf, строчными буквами.
fn task_name(path: &[u8]) -> FixedBuf<NAME_MAX> {
    let file = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
    let stem = if file.len() > 4 && file[file.len() - 4..].eq_ignore_ascii_case(b".elf") { &file[..file.len() - 4] } else { file };
    let mut name = FixedBuf::new();
    for &byte in stem.iter().take(NAME_MAX) { let _ = name.write_char(byte.to_ascii_lowercase() as char); }
    name
}

fn load(name: &[u8], init: usize, request: usize) -> Result<u64, Error> {
    let mut path = FixedBuf::<64>::new();
    path_for(name, &mut path)?;
    let task = task_name(path.as_bytes());
    // Сервисы запускает ядро из образов загрузчика; ядро как программа не запускается.
    if task.as_bytes() == b"kernel" || BOOT_SERVICES.iter().any(|s| s.as_bytes() == task.as_bytes()) { return Err(Error::NotFound); }
    let file = File::open(core::str::from_utf8(path.as_bytes()).unwrap())?;
    let size = file.size();
    if size < 64 || size > MAX_IMAGE { return Err(Error::Invalid); }
    let mut image = Pages::new(size).ok_or(Error::NoMemory)?;
    if file.read_at(0, &mut image.as_mut_slice()[..size])? != size || &image.as_slice()[..4] != b"\x7fELF" { return Err(Error::Invalid); }
    let cap = image.share()?;
    let result = mind::process::spawn_image(task.as_bytes(), cap, size, init, CAP_READ | CAP_WRITE | CAP_GRANT, request);
    let _ = ipc::drop_cap(cap); // ядро уже скопировало образ; буфер освобождается при выходе из функции
    result
}

// Текст для LIST: программы *.elf в корне диска, кроме ядра; сервисы помечены.
fn listing(out: &mut [u8]) -> usize {
    let mut at = 0;
    let _ = fs::list("", |entry| {
        if entry.is_dir || entry.name.len() < 5 || !entry.name[entry.name.len() - 4..].eq_ignore_ascii_case(b".elf") { return; }
        let name = task_name(entry.name);
        if name.as_bytes() == b"kernel" { return; }
        let mut line = FixedBuf::<64>::new();
        let service = BOOT_SERVICES.iter().any(|s| s.as_bytes() == name.as_bytes());
        let _ = writeln!(line, "  {:<12} {} BYTES{}", core::str::from_utf8(name.as_bytes()).unwrap_or("?"), entry.size, if service { " (SERVICE)" } else { "" });
        if at + line.as_bytes().len() <= out.len() { out[at..at + line.as_bytes().len()].copy_from_slice(line.as_bytes()); at += line.as_bytes().len(); }
    });
    at
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Ok(mut page) = Mapping::new(SLOT_MEM) else { mind::println!("[LOADER] NO REQUEST PAGE"); return };
    let _ = fs::prepare();
    mind::println!("[LOADER] READY");
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        if let Some(id) = request.kernel {
            // Запрос шелла: [вид, фон, длина имени, имя...] в странице ядра.
            let bytes = page.as_slice(); let (kind, len) = (bytes[0], (bytes[2] as usize).min(NAME_MAX * 4));
            let mut name = [0u8; NAME_MAX * 4]; name[..len].copy_from_slice(&bytes[3..3 + len]);
            let code = match kind {
                LOADER_RUN => match load(&name[..len], 0, id) { Ok(_) => 0, Err(error) => error.code() },
                LOADER_LIST => listing(&mut page.as_mut_slice()[LOADER_REPLY..]),
                _ => ERR_INVALID,
            };
            let _ = mind::process::loader_done(id, code);
        } else if request.is_call {
            // Запуск из программы: имя в двух словах сообщения, необязательный мандат для слота INIT ребёнка.
            let mut packed = [0u8; NAME_MAX];
            packed[..8].copy_from_slice(&request.data[0].to_le_bytes()); packed[8..].copy_from_slice(&request.data[1].to_le_bytes());
            let len = packed.iter().position(|&b| b == 0).unwrap_or(NAME_MAX);
            let result = load(&packed[..len], if request.cap_received { RECEIVED_CAP } else { 0 }, 0);
            if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
            let _ = ipc::reply(&Message::new(match result { Ok(pid) => pid as usize, Err(error) => error.code() }, 0));
        } else if request.cap_received {
            let _ = ipc::drop_cap(RECEIVED_CAP);
        }
    }
}
