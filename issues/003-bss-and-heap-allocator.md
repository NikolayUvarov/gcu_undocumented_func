# 003 — `.bss`, глобальные статики и куча (`linked_list_allocator`)

**Тип:** feature · **Приоритет:** высокий · **Статус:** open · **Блокируется:** [002](002-elf-loader.md)
**Затрагивает:** `kernel/`, `bootloader/` (передача карты памяти)

## Что требуется по handoff

Вернуть откаченную интеграцию: `extern crate alloc`, `#[global_allocator]` на `linked_list_allocator::LockedHeap`, `format!`/`String`/`Vec` в ядре.

## Текущее состояние

- В коде следов аллокатора нет (ни в `Cargo.toml`, ни в истории git).
- `linker.ld` объявляет `.bss`, но `llvm-objcopy -O binary` его не пишет, а `allocate_pages` считает страницы от размера файла → статики получат мусор либо выйдут за выделенную область. Подробно: [knowledge/03](../knowledge/03-flat-binary-layout-analysis.md), находка 3.

## План

1. Дождаться [002](002-elf-loader.md) (зануление `memsz - filesz`). Промежуточный вариант без ELF: символы `__bss_start`/`__bss_end` в `linker.ld`, вызов `write_bytes` из `_start` до любого обращения к статикам, и выделение `памяти = размер файла + размер .bss` (размер пробрасывать через `include_bytes!` второго файла или константу из build.rs).
2. Регион кучи: пока — статический массив в `.bss` (например 1 МБ) через `LockedHeap::init`. Далее — из карты памяти UEFI: в `bootloader` сохранить `MemoryMap` из `exit_boot_services`, отобрать `CONVENTIONAL` регионы и передать в `BootInfo` (указатель + количество дескрипторов + размер дескриптора).
3. Проверить, что `linked_list_allocator` собирается для `x86_64-unknown-none` без `std` (feature `use_spin`, но `spin` требует атомики — ок для x86_64).
4. `#[alloc_error_handler]` больше не нужен на актуальном nightly (default handler = panic); проверить на текущем тулчейне.

## Критерии готовности

- В ядре работает `format!("{}x{}", info.width, info.height)` и результат рисуется/проверяется.
- `static mut` инициализируются нулями (проверка: счётчик кадров стартует с 0).
- Размер `.bin`/ELF не «раздувается» нулями `.bss` (для ELF — `filesz < memsz`).
