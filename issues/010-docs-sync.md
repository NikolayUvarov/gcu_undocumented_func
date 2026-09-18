# 010 — Синхронизировать README и handoff с кодом

**Тип:** docs · **Приоритет:** средний · **Статус:** open
**Затрагивает:** `README.md`, handoff-документ

## Расхождения

| Где | Написано | Факт |
|---|---|---|
| README, «How to Build» | `chmod +x patch_008_preemptive.sh` / `./patch_008_preemptive.sh` | файла нет; сборка — `01_prepare_env.sh` + `02_build.sh` |
| Handoff, раздел 3 | «скрипт `fix_stable_boot.sh`» | файла нет; актуален `02_build.sh` |
| README, «Event Model» | «Hardware interrupts (IDT) for timing and keyboard input, combined with software interrupts (`int 0x80`)» | ни IDT, ни `int 0x80` в коде нет; клавиатура — поллинг портов; см. [004](004-apic-idt-interrupts.md), [005](005-syscalls.md) |
| README, «Toolchain» | «utilizing `naked_functions` and `abi_x86_interrupt`» | ни один из feature-гейтов не используется |
| README | путь `/usr/share/ovmf/OVMF.fd` | на Debian/Ubuntu: `/usr/share/OVMF/OVMF_CODE_4M.fd` (+ `OVMF_VARS_4M.fd`); в репозитории используется локальный `OVMF.fd` (gitignored) |
| README, заголовок | «Iain M. Banks supposed # MIND CORE» | склейка/опечатка |
| Handoff, раздел 1 | загрузчик читает файлы с FAT32 | `include_bytes!` — [006](006-bootloader-load-from-fat32.md) |
| Handoff, раздел 1 | рендер шрифта и примитивов | нет — [007](007-kernel-font-and-primitives.md) |
| Handoff, раздел 1 | переход в userspace по задержке или клавише | только по пробелу — [008](008-kernel-timeout-handoff.md) |
| Handoff, раздел 2 | «`.bss` без `linker.ld`» | `linker.ld` есть; проблема в objcopy/выделении — [003](003-bss-and-heap-allocator.md) |
| Handoff, раздел 3 | objcopy дампит `.text/.rodata/.data` | дампится всё alloc-содержимое — [001](001-flat-binary-entry-offset-and-got-call.md) |

## План

1. README: секцию «Event Model» переписать как «Step 0: polling; interrupts — roadmap», убрать несуществующий скрипт, добавить Linux-команду запуска QEMU с `OVMF_CODE_4M.fd` + `OVMF_VARS_4M.fd` (`-drive if=pflash,...`), исправить заголовок.
2. Положить handoff в репозиторий (`docs/handoff.md`) и обновить по матрице [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md).
3. Добавить в README ссылку на `knowledge/` и `issues/`.
4. Добавить `03_run_qemu.sh` для Linux (парный к `.bat`).

## Критерии готовности

Каждое утверждение README/handoff либо соответствует коду, либо помечено как «roadmap» со ссылкой на issue.
