// MIND Core's page side (215-PRT-0017), the same in a tab and in Node: it lays the kernel module's memory out as a loader
// would, starts the kernel, and runs its turns. The kernel fills BootInfo itself from the words `start` writes.
export const BOOT_FILES = ["init.elf", "logd.elf", "rtc.elf", "ps2_kbd.elf", "virtio_input.elf", "compositor.elf", "ata.elf", "ahci.elf", "usb_host.elf", "usb_storage.elf", "usb_hid.elf", "virtio_blk.elf", "nvme.elf", "ramdisk.elf", "vfs_server.elf", "usb_serial.elf", "uart_serial.elf", "link.elf", "blockstore.elf", "gpio.elf", "battery.elf", "loader.elf", "audio_gw.elf", "tts.elf", "video_gw.elf", "virtio_net.elf", "rtl8168.elf", "rtw89_wifi.elf", "rtw88_wifi.elf", "bt_hci.elf", "bcm_wifi.elf", "tpm.elf", "keystore.elf", "storefs.elf", "wlansim.elf", "wlan.elf", "netstack.elf", "netpolicy.elf", "timed.elf", "parse.elf", "tls.elf", "ssh.elf", "windows.elf", "sysmon.elf", "nnd.elf", "updater.elf", "effector_gw.elf", "effector.elf", "httpd.elf", "sshd.elf", "clipboard.elf", "display.elf", "shell.elf"];

const PAGE = 4096;
const up = (n, to) => Math.ceil(n / to) * to;

/// Starts the kernel. `kernel`: the wasm32 module's bytes; `files`: a Map of BOOT_FILES names to their bytes;
/// `volume`: the boot volume's FAT image, or none; `write(byte)`: the console; `stopped(why)`: 0 off, 1 restart, 2 fault.
export async function start({ kernel, files, volume, write, stopped, memoryMiB = 512, heapMiB = 32, width = 1024, height = 768 }) {
    let halted = null;
    const word = new BigUint64Array(1);
    const { instance } = await WebAssembly.instantiate(kernel, { mind: {
        console_write: (byte) => write(byte),
        now_ns: () => BigInt(Math.floor(performance.now() * 1e6)),
        halt: (why) => { halted = why; stopped?.(why); },
        random: () => { crypto.getRandomValues(word); return word[0]; },
    } });
    const e = instance.exports, memory = e.memory;
    const total = memoryMiB << 20;
    memory.grow((total - memory.buffer.byteLength) / 65536);
    let at = up(Number(e.mind_heap_base()), 65536);
    const take = (bytes) => { const p = at; at = up(at + Math.max(bytes, 1), PAGE); return p; };
    const put = (bytes) => { const p = take(bytes.length); new Uint8Array(memory.buffer, p, bytes.length).set(bytes); return [p, bytes.length]; };
    const words = take(PAGE);
    const screen = take(width * height * 4);
    const heap = take(heapMiB << 20);
    const images = BOOT_FILES.map((name) => files.has(name) ? put(files.get(name)) : [0, 0]);
    const live = volume ? put(volume) : [0, 0];
    const pool = up(at, PAGE);
    if (pool + (64 << 20) > total) throw new Error(`${memoryMiB} MiB leave no room for the frame pool`);
    const w = new BigUint64Array(memory.buffer, words, 11 + 2 * BOOT_FILES.length);
    [screen, width, height, width, heap, heapMiB << 20, pool, total - pool, live[0], live[1], BOOT_FILES.length].forEach((v, i) => { w[i] = BigInt(v); });
    images.forEach(([p, n], i) => { w[11 + 2 * i] = BigInt(p); w[12 + 2 * i] = BigInt(n); });
    e.mind_start(BigInt(words));
    return {
        /// Runs the kernel for about `ms`: 0 when it has more to do at once, else the milliseconds to its next tick.
        turn: (ms) => (halted === null ? Number(e.mind_turn(BigInt(ms))) : -1),
        type: (bytes) => { for (const b of bytes) e.mind_console_in(b); },
        /// The screen's pixels, BGRX, as the kernel's framebuffer holds them.
        screen: () => new Uint8Array(memory.buffer, screen, width * height * 4),
        width, height, halted: () => halted,
    };
}
