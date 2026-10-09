#![no_std]
#![no_main]
// efivar: the firmware's boot variables (351-KRN-0027): the boot entries, the order, the entry booted now and the one
// to boot next once. A console program; the shell lends the firmware privilege after asking the user.
use mind::abi::BootInfo;
use mind::firmware::{self, APPEND, AUTHENTICATED, BOOT_VARIABLE, GLOBAL, IMAGE_SECURITY};
use mind::sys::Error;

mind::request!(REQUEST_CONSOLE | REQUEST_FIRMWARE);

// A boot option number from four hex digits.
fn option(text: &str) -> Option<u16> { (text.len() <= 4).then(|| u16::from_str_radix(text, 16).ok()).flatten() }

// A variable's u16 values (BootCurrent, BootNext, BootOrder).
fn numbers(name: &str) -> core::result::Result<([u16; 64], usize), Error> {
    let mut data = [0u8; 128];
    let (_, length) = firmware::get(name, &GLOBAL, &mut data)?;
    let mut values = [0u16; 64];
    let count = (length.min(data.len()) / 2).min(values.len());
    for (i, value) in values[..count].iter_mut().enumerate() { *value = u16::from_le_bytes([data[2 * i], data[2 * i + 1]]); }
    Ok((values, count))
}

fn show(name: &str) {
    match numbers(name) {
        Ok((values, count)) => { mind::print!("{}:", name); for v in &values[..count] { mind::print!(" {:04X}", v); } mind::println!(); }
        Err(Error::NotFound) => mind::println!("{}: not set", name),
        Err(e) => mind::println!("{}: {:?}", name, e),
    }
}

// Boot####: attributes (u32), the device path's length (u16), the description (UTF-16, NUL-terminated), the path.
fn entry(number: u16) -> Option<()> {
    let mut name = mind::util::FixedBuf::<8>::new();
    let _ = core::fmt::Write::write_fmt(&mut name, format_args!("Boot{:04X}", number));
    let mut data = [0u8; 1024];
    let (_, length) = firmware::get(core::str::from_utf8(name.as_bytes()).ok()?, &GLOBAL, &mut data).ok()?;
    let data = &data[..length.min(data.len())];
    let active = data.first().is_some_and(|a| a & 1 != 0);
    mind::print!("Boot{:04X}{} ", number, if active { "" } else { " (inactive)" });
    for unit in data.get(6..)?.chunks_exact(2).map(|u| u16::from_le_bytes([u[0], u[1]])).take_while(|&u| u != 0) {
        mind::print!("{}", char::from_u32(unit as u32).unwrap_or('?'));
    }
    mind::println!();
    Some(())
}

fn set_numbers(name: &str, values: &[u16]) {
    let mut data = [0u8; 128];
    for (i, v) in values.iter().enumerate() { data[2 * i..2 * i + 2].copy_from_slice(&v.to_le_bytes()); }
    match firmware::set(name, &GLOBAL, BOOT_VARIABLE, &data[..2 * values.len()]) {
        Ok(()) => { mind::print!("{} SET TO", name); for v in values { mind::print!(" {:04X}", v); } mind::println!(); }
        Err(e) => mind::println!("efivar: {}: {:?}", name, e),
    }
}

// Appends a signed signature list from `path` (EFI_VARIABLE_AUTHENTICATION_2 and the list, as sbvarsign writes it) to
// db, dbx or KEK: the firmware takes it only if a key it trusts signed it (351-KRN-0028).
fn append(name: &str, path: &str) {
    let guid = match name { "db" | "dbx" => IMAGE_SECURITY, "KEK" => GLOBAL, _ => { mind::println!("efivar: append takes db, dbx or KEK"); return; } };
    let file = match mind::fs::File::open(path) { Ok(file) => file, Err(e) => { mind::println!("efivar: {}: {:?}", path, e); return; } };
    let most = mind::abi::FIRMWARE_BUFFER - mind::abi::FIRMWARE_HEADER - 2 * name.len();
    if file.size() == 0 || file.size() > most { mind::println!("efivar: {}: {} bytes, 1 to {}", path, file.size(), most); return; }
    let Some(mut pages) = mind::mem::Pages::new(file.size()) else { mind::println!("efivar: out of memory"); return };
    let data = pages.as_mut_slice();
    let length = match file.read_at(0, &mut data[..file.size()]) { Ok(n) => n, Err(e) => { mind::println!("efivar: {}: {:?}", path, e); return; } };
    match firmware::set(name, &guid, BOOT_VARIABLE | AUTHENTICATED | APPEND, &data[..length]) {
        Ok(()) => mind::println!("{} APPENDED: {} BYTES", name, length),
        Err(e) => mind::println!("efivar: {}: {:?}", name, e),
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("efivar — the firmware's boot variables.\nUsage: efivar [boot] | bootnext <hex> | bootorder <hex>,<hex>... | delete bootnext | append db|dbx|KEK <file>\nboot: the boot entries, the order, the entry booted now and the next one.\nappend: a signed signature list (sbvarsign's output) added to db, dbx or KEK.");
    let args = mind::process::args_str();
    let mut words = args.split_whitespace();
    match (words.next(), words.next()) {
        (None | Some("boot"), None) => {
            if mind::dev::cap_info(mind::abi::SLOT_FIRMWARE).0 != mind::abi::CAP_KIND_FIRMWARE { mind::println!("efivar: the firmware's variables were not granted"); return; }
            show("BootCurrent"); show("BootNext"); show("BootOrder");
            for number in 0..=0x20u16 { entry(number); }
        }
        (Some("bootnext"), Some(text)) => match option(text) { Some(n) => set_numbers("BootNext", &[n]), None => mind::println!("efivar: not a boot option: {}", text) },
        (Some("bootorder"), Some(text)) => {
            let (mut values, mut count) = ([0u16; 64], 0);
            for part in text.split(',') {
                match option(part) { Some(n) if count < values.len() => { values[count] = n; count += 1; } _ => { mind::println!("efivar: not a boot option: {}", part); return; } }
            }
            set_numbers("BootOrder", &values[..count]);
        }
        (Some("delete"), Some("bootnext")) => match firmware::set("BootNext", &GLOBAL, BOOT_VARIABLE, &[]) {
            Ok(()) | Err(Error::NotFound) => mind::println!("BootNext DELETED"),
            Err(e) => mind::println!("efivar: BootNext: {:?}", e),
        },
        (Some("append"), Some(name)) => match words.next() { Some(path) => append(name, path), None => mind::println!("Usage: efivar append db|dbx|KEK <file>") },
        _ => mind::println!("Usage: efivar [boot] | bootnext <hex> | bootorder <hex>,<hex>... | delete bootnext | append db|dbx|KEK <file>"),
    }
}
