//! Рантайм программ: обработчик паники (журнал + выход вместо вечного цикла) и mem*-функции.
use core::ffi::c_void;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut text = crate::util::FixedBuf::<256>::new();
    let _ = core::fmt::write(&mut text, format_args!("PANIC: {}\n", info.message()));
    crate::process::log(text.as_bytes());
    crate::process::exit()
}

// Цель x86_64-unknown-none не даёт mem*-символов; volatile не даёт компилятору свернуть цикл в вызов самого себя.
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void {
    for i in 0..n { core::ptr::write_volatile((s as *mut u8).add(i), c as u8); }
    s
}
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    for i in 0..n { core::ptr::write_volatile((dest as *mut u8).add(i), core::ptr::read_volatile((src as *const u8).add(i))); }
    dest
}
#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    if (dest as usize) <= (src as usize) { return memcpy(dest, src, n); }
    for i in (0..n).rev() { core::ptr::write_volatile((dest as *mut u8).add(i), core::ptr::read_volatile((src as *const u8).add(i))); }
    dest
}
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    for i in 0..n {
        let (a, b) = (core::ptr::read_volatile((s1 as *const u8).add(i)), core::ptr::read_volatile((s2 as *const u8).add(i)));
        if a != b { return a as i32 - b as i32; }
    }
    0
}
#[no_mangle]
pub unsafe extern "C" fn bcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 { memcmp(s1, s2, n) }
