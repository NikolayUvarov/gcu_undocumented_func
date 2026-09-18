# Сопоставление handoff-документа с кодом

Легенда: ✅ есть в коде и соответствует; ⚠️ есть частично или иначе; ❌ отсутствует.

## Раздел 1. «Текущее состояние»

| Утверждение handoff | Статус | Что в коде | Задача |
|---|---|---|---|
| Загрузчик инициализирует GOP, получает фреймбуфер/разрешение/stride | ✅ | `bootloader/src/main.rs:35-45` | — (см. риск по PixelFormat → 009) |
| Загрузчик считывает ядро и приложение с FAT32 | ❌ | `include_bytes!` в EFI, файловая система не открывается | [006](../issues/006-bootloader-load-from-fat32.md) |
| Плоские бинарники через `llvm-objcopy`, секции `.text/.rodata/.data` | ⚠️ | `-O binary` без `-j` — в файл попадают `.dynsym/.hash/.dynamic/.got` | [001](../issues/001-flat-binary-entry-offset-and-got-call.md) |
| Ядро получает управление от загрузчика | ⚠️ | прыжок на смещение 0, где у ядра не `_start` | [001](../issues/001-flat-binary-entry-offset-and-got-call.md) |
| Базовое графическое ядро: процедурный рендер шрифта и примитивов | ❌ | только попиксельный круг; ни шрифта, ни примитивов | [007](../issues/007-kernel-font-and-primitives.md) |
| Поллинг портов `0x64`/`0x60` | ✅ | `kernel/src/main.rs:24-25` | — |
| Передача в userspace по истечении задержки **или** по клавише | ⚠️ | только по пробелу (`0x39`); таймаута нет | [008](../issues/008-kernel-timeout-handoff.md) |
| Userspace принимает указатели на буфер и перехватывает отрисовку | ✅ | `app/src/main.rs`, вращающийся квадрат | — |

## Раздел 2. «Пройденные этапы» (откачено)

| Утверждение | Статус | Комментарий | Задача |
|---|---|---|---|
| `linked_list_allocator`, `format!` были интегрированы, откачены из-за `.bss` | ❌ в коде | зависимостей `alloc`/аллокатора нет; в git-истории (2 коммита) следов нет | [003](../issues/003-bss-and-heap-allocator.md) |
| «`.bss` без `linker.ld`» | ⚠️ | `linker.ld` существует и объявляет `.bss`; проблема в objcopy и выделении страниц, а не в отсутствии скрипта | [003](../issues/003-bss-and-heap-allocator.md) |
| IDT, ремап PIC, PIT/PS2, переключение контекста — откачено из-за Triple Fault | ❌ в коде | нет ни IDT, ни `x86-interrupt` ABI, ни asm-переключения | [004](../issues/004-apic-idt-interrupts.md) |

## Раздел 3. «Сборочная инфраструктура»

| Утверждение | Статус | Комментарий | Задача |
|---|---|---|---|
| Пайплайн `fix_stable_boot.sh` | ❌ | файла нет; актуален `02_build.sh`; README ссылается на несуществующий `patch_008_preemptive.sh` | [010](../issues/010-docs-sync.md) |
| Шаги 1–4 пайплайна | ✅ | совпадают с `02_build.sh` (с оговоркой про `-j`) | [001](../issues/001-flat-binary-entry-offset-and-got-call.md) |

## Раздел 4. Roadmap

| Пункт | Статус | Задача |
|---|---|---|
| ELF-загрузчик (приоритет) | ❌ | [002](../issues/002-elf-loader.md) |
| APIC вместо PIC | ❌ | [004](../issues/004-apic-idt-interrupts.md) |
| Syscalls (`int 0x80`) | ❌ | [005](../issues/005-syscalls.md) |

## Не упомянуто в handoff, но найдено при ревизии

| Находка | Задача |
|---|---|
| GOP `PixelFormat` не проверяется, ядро жёстко пишет BGRX | [009](../issues/009-gop-pixel-format.md) |
| Нет `rust-toolchain.toml`, `Cargo.lock` игнорируется, `01_prepare_env.sh` меняет глобальный default | [011](../issues/011-reproducible-toolchain.md) |
| README заявляет IDT и `int 0x80` как реализованные | [010](../issues/010-docs-sync.md) |
