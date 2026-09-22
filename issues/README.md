# issues/ — задачи на доработку

Создано по итогам ревизии 2026-09-17 (сопоставление handoff ↔ код, см. [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md)).

| № | Задача | Тип | Приоритет | Блокируется |
|---|---|---|---|---|
| [001](001-flat-binary-entry-offset-and-got-call.md) | `_start` не по смещению 0; `memset` через нулевой GOT | bug | критический | — |
| [002](002-elf-loader.md) | ELF-загрузчик вместо плоских бинарников | feature (roadmap 1) | высокий | — |
| [003](003-bss-and-heap-allocator.md) | `.bss`, статики, куча `linked_list_allocator` | feature | высокий | 002 |
| [004](004-apic-idt-interrupts.md) | IDT + APIC, таймер и клавиатура по прерываниям | feature (roadmap 2) | высокий | 003 |
| [005](005-syscalls.md) | Syscalls `int 0x80`, общий крейт ABI | feature (roadmap 3) | средний | 004 |
| [006](006-bootloader-load-from-fat32.md) | Чтение kernel/app с FAT32 | feature | средний | — |
| [007](007-kernel-font-and-primitives.md) | Шрифт, примитивы, консоль, panic-вывод | feature | средний | — |
| [008](008-kernel-timeout-handoff.md) | Переход в userspace по таймауту | feature | низкий | — |
| [009](009-gop-pixel-format.md) | Учёт `PixelFormat`, выбор режима GOP | bug/robustness | средний | — |
| [010](010-docs-sync.md) | Синхронизация README/handoff с кодом | docs | средний | — |
| [011](011-reproducible-toolchain.md) | `rust-toolchain.toml`, `Cargo.lock`, workspace, CI | infra | низкий | — |
| [012](../issues-done/012-multitasking-and-program-instances.done) | Многозадачность, независимые экземпляры, `ps`/`kill`/`fg` | feature — выполнено 2026-09-18 | высокий | — |
| [013](../issues-done/013-smp-and-memory-isolation.done) | SMP, ring 3 и аппаратная изоляция памяти | feature — выполнено 2026-09-19 | высокий | — |
| [014](../issues-done/014-private-program-heap.done) | Приватная динамическая память программ | feature — выполнено 2026-09-19 | высокий | — |

Рекомендуемый порядок: 001 → 007 (диагностика на экране) → 002 → 003 → 004 → 005; 006/009/010/011 — параллельно.

Формат файла задачи: заголовок, блок метаданных (тип/приоритет/статус/блокировки), «Проблема/Расхождение», «План», «Критерии готовности», «Связано». При закрытии — статус в заголовке и в этой таблице.

Выполненные задачи 012–014 перенесены в `issues-done/` с расширением `.done`. Постановки и результаты проверок сохранены в файлах.
