# 001 — Плоские бинарники: `_start` не по смещению 0, `memset` вызывается по адресу 0

**Тип:** bug · **Приоритет:** критический · **Статус:** open
**Затрагивает:** `kernel/linker.ld`, `app/linker.ld`, `*/.cargo/config.toml`, `02_build.sh`

## Проблема

Handoff описывает пайплайн как «дамп секций `.text/.rodata/.data`», но `02_build.sh` вызывает `llvm-objcopy -O binary` без `-j`, а `linker.ld` не управляет служебными PIE-секциями. В результате (подтверждено `readelf`/`xxd`, детали в [knowledge/03](../knowledge/03-flat-binary-layout-analysis.md)):

1. У ядра `.dynsym/.gnu.hash/.hash/.dynstr/.rela.dyn` размещены **перед** `.text`; `_start` — по смещению `0x60`, а загрузчик прыгает на `kernel_addr + 0`. Исполняются 96 байт хеш-таблиц. Работает только если в `rax` случайно оказался выровненный адрес.
2. `core::ptr::write_bytes` компилируется в `call *memset@GOT(%rip)`; слот GOT в `.bin` содержит `0`, релокацию `R_X86_64_RELATIVE` никто не применяет. Вызов уходит по физическому адресу `0`. То же в app.

## Критерии готовности

- `readelf -h kernel` и `readelf -h app` показывают `Entry point address: 0x0`.
- `readelf -r` для обоих: `There are no relocations in this file`.
- `llvm-objdump -d` не содержит `callq *...(%rip)` в адрес `.got`.
- `xxd -l 4 kernel.bin` начинается с кода `_start` (`55 41 57 41` для текущего кода).
- Запуск в QEMU: пульсирующий круг, по пробелу — квадрат.

## Проверенное решение (собрано в scratchpad, в QEMU не запускалось)

1. `.cargo/config.toml` (оба крейта):
   ```toml
   rustflags = ["-C", "link-arg=-Tlinker.ld", "-C", "relocation-model=pic", "-Z", "relax-elf-relocations=yes"]
   ```
   Флаг nightly-only; проект и так на nightly.
2. `linker.ld` (оба крейта) — после `.bss` добавить:
   ```ld
   .dynsym   : { *(.dynsym) }
   .gnu.hash : { *(.gnu.hash) }
   .hash     : { *(.hash) }
   .dynstr   : { *(.dynstr) }
   .rela.dyn : { *(.rela.dyn) *(.rela.*) }
   .dynamic  : { *(.dynamic) }
   .got      : { *(.got .got.*) }
   /DISCARD/ : { *(.eh_frame) *(.comment) }
   ```
3. `02_build.sh`:
   ```bash
   $OBJCOPY -O binary -j .text -j .rodata -j .data kernel/target/.../kernel bootloader/src/kernel.bin
   $OBJCOPY -O binary -j .text -j .rodata -j .data app/target/.../app       bootloader/src/app.bin
   ```
4. Добавить в `02_build.sh` самопроверку после сборки: `readelf -h ... | grep -q 'Entry point address: *0x0'` и `readelf -r ... | grep -q 'no relocations'`, иначе `exit 1`.

Ожидаемые размеры после правки: `kernel.bin` ≈ 565 байт, `app.bin` ≈ 808 байт.

## Связано

- Полностью снимается issue [002](002-elf-loader.md) (ELF-загрузчик применяет релокации сам), но до него это исправление нужно как стабилизирующее.
