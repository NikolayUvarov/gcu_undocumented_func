# 002 — ELF-загрузчик вместо плоских бинарников

**Тип:** feature (roadmap №1, приоритет по handoff) · **Приоритет:** высокий · **Статус:** open
**Затрагивает:** `bootloader/`, `02_build.sh`, `kernel/linker.ld`, `app/linker.ld`

## Что требуется по handoff

Отказаться от `objcopy` и научить загрузчик (первый шаг — UEFI-загрузчик, не ядро) разбирать ELF64: читать `Elf64_Ehdr`, обходить `PT_LOAD`, выделять `p_memsz` страниц, копировать `p_filesz`, занулять хвост (`.bss`), применять `R_X86_64_RELATIVE` из `PT_DYNAMIC`/`.rela.dyn`, прыгать на `e_entry`.

## Текущее состояние

- Загрузчик встраивает готовые `.bin` через `include_bytes!` и прыгает на смещение 0 (`bootloader/src/main.rs:20-33, 56`).
- ELF-файлы уже PIE (`ET_DYN`) с одной `R_X86_64_RELATIVE` — формат готов к такой загрузке.

## План

1. В kernel/app: убрать `-Tlinker.ld`-хаки или оставить, но не требовать `. = 0`; выровнять `PT_LOAD` по 4 КБ (`-z separate-code` уже даёт).
2. В `02_build.sh`: шаг objcopy убрать; в bootloader встраивать (или читать с FAT32 — issue 006) сами ELF.
3. В bootloader: модуль `elf.rs` без внешних зависимостей (или крейт `goblin`/`elf` с `no_std`+`default-features=false`; проверить сборку под `x86_64-unknown-uefi`).
   - валидация магии, `EM_X86_64`, `ET_DYN|ET_EXEC`;
   - для каждого `PT_LOAD`: `allocate_pages(AnyPages, LOADER_DATA, ceil(memsz/4096))` для ET_DYN одним блоком под весь образ (min vaddr..max vaddr), копия `filesz`, `write_bytes(0)` до `memsz`;
   - релокации: пройти `.rela.dyn` (по `PT_DYNAMIC` → `DT_RELA/DT_RELASZ`), для `R_X86_64_RELATIVE`: `*(base + r_offset) = base + r_addend`; на другие типы — паника с выводом в stdout до `exit_boot_services`;
   - вход: `base + e_entry`.
4. `BootInfo`: добавить `kernel_base`, `app_base`, `app_entry` (сейчас `app_ptr` совмещает базу и точку входа).
5. Тест: `readelf -l` показывает `.bss` с `memsz > filesz`, а `static mut COUNTER: u64` в ядре после загрузки равен 0 и корректно инкрементируется.

## Критерии готовности

- Ядро с `static mut` и `#[global_allocator]` (issue 003) запускается в QEMU.
- `02_build.sh` не содержит `llvm-objcopy`.
- Загрузчик отвергает битый ELF с сообщением на экране, а не молча зависает.

## Связано

[001](001-flat-binary-entry-offset-and-got-call.md) (временный фикс), [003](003-bss-and-heap-allocator.md) (разблокируется), [006](006-bootloader-load-from-fat32.md).
