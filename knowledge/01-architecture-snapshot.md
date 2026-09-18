# Срез архитектуры (по коду, коммит 8ad7550)

## Компоненты

| Компонент | Путь | Таргет | Формат | Размер артефакта |
|-----------|------|--------|--------|------------------|
| Bootloader | `bootloader/` | `x86_64-unknown-uefi` | PE/COFF (`BOOTX64.EFI`) | 16 896 байт |
| Kernel | `kernel/` | `x86_64-unknown-none` | плоский бинарник `kernel.bin` | 896 байт |
| App (userspace) | `app/` | `x86_64-unknown-none` | плоский бинарник `app.bin` | 1 136 байт |

Три независимых cargo-пакета без workspace. Общего крейта для типов нет.

## Контракт `BootInfo`

Структура продублирована три раза (bootloader/kernel/app), `#[repr(C)]`, одинаковая:

```rust
pub struct BootInfo {
    pub fb_ptr: *mut u8,   // адрес фреймбуфера GOP
    pub width: usize,
    pub height: usize,
    pub stride: usize,     // в пикселях, не в байтах
    pub app_ptr: *const u8 // физический адрес загруженного app.bin
}
```

Соглашение вызова: `extern "sysv64" fn(&BootInfo) -> !`. Указатель на `BootInfo` лежит на стеке загрузчика и живёт вечно, так как загрузчик не возвращается.

Риск: дублирование без единого источника — при изменении полей в одном месте контракт ломается молча (см. [05-observations-and-risks.md](05-observations-and-risks.md)).

## Поток управления

1. `bootloader/src/main.rs:20-21` — `kernel.bin` и `app.bin` встроены в EFI через `include_bytes!` (не читаются с FAT32).
2. `:27-33` — `allocate_pages(AnyPages, LOADER_DATA, len/4096+1)` для каждого бинарника, `copy_nonoverlapping`.
3. `:35-37` — GOP: `get_handle_for_protocol` + `open_protocol_exclusive`, берётся текущий режим без выбора и без проверки `PixelFormat`.
4. `:53` — `exit_boot_services(LOADER_DATA)`; карта памяти отбрасывается.
5. `:56-57` — `transmute(kernel_addr)` и прыжок по смещению **0** плоского бинарника (см. [03](03-flat-binary-layout-analysis.md): в текущей сборке `_start` там не лежит).
6. `kernel/src/main.rs:18-62` — бесконечный цикл: поллинг порта `0x64`, при бите 0 чтение `0x60`; сканкод `0x39` (пробел) → `transmute(info.app_ptr)` и вызов приложения. Иначе рисуется пульсирующий круг, задержка — `1_000_000` `nop`.
7. `app/src/main.rs:20-54` — бесконечный цикл: вращающийся квадрат по табличному синусу. Назад в ядро не возвращается.

## Чего в коде нет (при этом упоминается в README/handoff)

- IDT, ремап PIC, обработчики IRQ0/IRQ1, переключение контекста, `int 0x80` — отсутствуют полностью.
- Глобальный аллокатор, `alloc`, `format!` — отсутствуют.
- Чтение файлов с FAT32 через `SimpleFileSystem` — отсутствует.
- Рендер шрифта/примитивов — отсутствует (только круг и квадрат попиксельно).
- Передача управления в userspace по таймауту — отсутствует (только по клавише).
- Собственный стек ядра, `.bss`-зануление, обработка карты памяти — отсутствуют.

Всё это оформлено как задачи в `issues/`, см. [04-handoff-vs-code-matrix.md](04-handoff-vs-code-matrix.md).
