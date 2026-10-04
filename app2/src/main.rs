#![no_std]
#![no_main]

use mind::abi::BootInfo;
use mind::font::FONT;
use mind::util::Decimal;

const BACKGROUND: u32 = 0x001E1E2E;
const FOREGROUND: u32 = 0x00A6E3A1;
const FRAME_INTERVAL_MS: usize = 30;

fn os_print(message: &[u8]) { mind::process::log(message) }

fn fill_rect(info: &BootInfo, x: usize, y: usize, width: usize, height: usize, color: u32) {
    for py in y..y.saturating_add(height).min(info.height) {
        for px in x..x.saturating_add(width).min(info.width) {
            unsafe { core::ptr::write_volatile(info.fb_ptr.add(py * info.stride + px), color) };
        }
    }
}

fn draw_text(info: &BootInfo, x: usize, y: usize, text: &[u8], color: u32) {
    for (index, &ch) in text.iter().enumerate() {
        let glyph = FONT[ch.to_ascii_uppercase().saturating_sub(32).min(63) as usize];
        for row in 0..8 {
            for col in 0..8 {
                let px = x + index * 8 + col;
                let py = y + row;
                if px < info.width
                    && py < info.height
                    && glyph & (1 << ((7 - row) * 8 + 7 - col)) != 0
                {
                    unsafe {
                        core::ptr::write_volatile(info.fb_ptr.add(py * info.stride + px), color)
                    };
                }
            }
        }
    }
}

fn bounce(frame: usize, limit: usize) -> usize {
    if limit == 0 {
        return 0;
    }
    let phase = frame % (limit * 2);
    if phase <= limit {
        phase
    } else {
        limit * 2 - phase
    }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    os_print(
        b"\r\n[APP2] HELLO FROM THE SECOND ELF PROGRAM!\r\n",
    );
    os_print(
        b"[APP2] BOUNCING SQUARE. CTRL+Z: SHELL, ESC: EXIT.\r\n",
    );

    // All state is local so every RUN APP2 starts a fresh animation and counter.
    let mut frame: usize = 0;
    let mut last_frame_ms = mind::time::uptime_ms().wrapping_sub(FRAME_INTERVAL_MS);
    let top = 120.min(info.height);
    let size = 64
        .min(info.width.saturating_sub(48))
        .min(info.height.saturating_sub(top + 24));
    let Some(mut sprite) = mind::mem::Pages::new(size * size * 4) else {
        os_print(b"[APP2] OUT OF MEMORY FOR SPRITE. EXITING.\r\n");
        return;
    };
    for pixel in sprite.as_mut_slice().chunks_exact_mut(4) {
        pixel.copy_from_slice(&FOREGROUND.to_le_bytes());
    }
    os_print(b"[APP2] PRIVATE HEAP SPRITE READY (RW+NX).\r\n");
    let travel_x = info.width.saturating_sub(size + 48);
    let travel_y = info.height.saturating_sub(top + size + 24);
    let mut previous_square = None;
    fill_rect(info, 0, 0, info.width, info.height, BACKGROUND);

    loop {
        if mind::input::read_key().is_some_and(mind::input::is_escape) {
            os_print(b"[APP2] ESC PRESSED. RETURNING TO KERNEL.\r\n");
            return;
        }

        let now = mind::time::uptime_ms();
        let elapsed = now.wrapping_sub(last_frame_ms);
        if elapsed < FRAME_INTERVAL_MS {
            mind::time::sleep(FRAME_INTERVAL_MS - elapsed);
            continue;
        }
        last_frame_ms = now;
        let ticks = mind::time::rdtsc() as usize;
        let frame_decimal = Decimal::new(frame);
        let frame_text = frame_decimal.as_bytes();
        let tick_decimal = Decimal::new(ticks);
        let tick_text = tick_decimal.as_bytes();

        if let Some((x, y)) = previous_square {
            fill_rect(info, x, y, size, size, BACKGROUND);
        }
        fill_rect(info, 96, 72, 160, 8, BACKGROUND);
        fill_rect(info, 96, 96, 160, 8, BACKGROUND);
        draw_text(info, 24, 24, b"SECOND APP (ELF) - RUN APP2", FOREGROUND);
        draw_text(info, 24, 48, b"CTRL+Z: SHELL / ESC: EXIT", 0x00FFFFFF);
        draw_text(info, 24, 72, b"FRAMES:", FOREGROUND);
        draw_text(info, 96, 72, frame_text, 0x00FFFFFF);
        draw_text(info, 24, 96, b"TSC:", FOREGROUND);
        draw_text(info, 96, 96, tick_text, 0x00FFFFFF);
        let square_x = 24 + bounce(frame, travel_x);
        let square_y = top + bounce(frame, travel_y);
        for (index, pixel) in sprite.as_mut_slice().chunks_exact(4).enumerate() {
            let x = square_x + index % size;
            let y = square_y + index / size;
            if x < info.width && y < info.height {
                unsafe {
                    info.fb_ptr
                        .add(y * info.stride + x)
                        .write_volatile(u32::from_le_bytes(pixel.try_into().unwrap()));
                }
            }
        }
        previous_square = Some((24 + bounce(frame, travel_x), top + bounce(frame, travel_y)));

        // Log periodically instead of flooding the UART on every redraw.
        if frame % 30 == 0 {
            os_print(b"[APP2] FRAME=");
            os_print(frame_text);
            os_print(b" TSC=");
            os_print(tick_text);
            os_print(b"\r\n");
        }

        frame = frame.wrapping_add(1);
    }
}

