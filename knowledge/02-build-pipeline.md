# Сборочная инфраструктура

## Тулчейн на машине ревизии (2026-09-17)

```
rustc 1.100.0-nightly (574ff7d98 2026-09-14)
cargo 1.100.0-nightly (7941be6fb 2026-09-11)
targets: x86_64-unknown-linux-gnu, x86_64-unknown-none, x86_64-unknown-uefi
llvm-tools-preview: установлен (llvm-objcopy, llvm-objdump; llvm-readelf отсутствует — используется системный readelf)
qemu-system-x86_64: НЕ установлен в WSL; запуск делается на Windows через 03_run_qemu_windows.bat
OVMF: в корне проекта OVMF.fd (4 МБ, gitignored); системный /usr/share/OVMF/OVMF_CODE_4M.fd
```

Проверено: `./02_build.sh` проходит без ошибок и без предупреждений компилятора во всех трёх крейтах.

## Скрипты

| Скрипт | Назначение | Замечания |
|--------|-----------|-----------|
| `01_prepare_env.sh` | ставит rustup, делает `rustup default nightly` **глобально**, добавляет таргеты и llvm-tools | меняет дефолтный тулчейн пользователя; нет `rust-toolchain.toml` → issue 011 |
| `02_build.sh` | kernel/app → `cargo build --release` → `llvm-objcopy -O binary` → bootloader → копия в `usb_root/EFI/BOOT/BOOTX64.EFI` | objcopy вызывается **без** `-j .text -j .rodata -j .data`, поэтому в `.bin` попадают все alloc-секции, включая `.dynsym/.hash/.dynamic/.got` → issue 001 |
| `03_run_qemu_windows.bat` | `qemu-system-x86_64.exe -bios OVMF.fd -drive format=raw,file=fat:rw:usb_root -m 512` | Linux-варианта нет |

Скрипты, на которые ссылаются документы, но которых нет в репозитории: `patch_008_preemptive.sh` (README), `fix_stable_boot.sh` (handoff). Актуальный — `02_build.sh`. → issue 010.

## Флаги компиляции kernel/app

`.cargo/config.toml` (одинаков для kernel и app):
```toml
[build]
target = "x86_64-unknown-none"
rustflags = ["-C", "link-arg=-Tlinker.ld", "-C", "relocation-model=pic"]
```

Факты о таргете `x86_64-unknown-none` (из `rustc --print target-spec-json`):
- `position-independent-executables: true`, `static-position-independent-executables: true` — PIE уже по умолчанию, флаг `relocation-model=pic` избыточен, но безвреден.
- `code-model: kernel`, `relro-level: full`, `panic-strategy: abort`, SSE/MMX выключены (`+soft-float`).
- `relax-elf-relocations` по умолчанию **выключен** → LLD не сворачивает `call *memset@GOTPCREL(%rip)` в прямой `call` → см. issue 001.

## `linker.ld` (kernel и app идентичны)

Секции `.text` (сначала `.text._start`), `.rodata`, `.data`, `.bss`, `/DISCARD/ .eh_frame`. Служебные PIE-секции не перечислены, поэтому LLD размещает их как orphan по своим правилам ранга — у ядра **перед** `.text` (см. [03](03-flat-binary-layout-analysis.md)).

Handoff утверждает, что `.bss` пытались включить «без скрипта линковщика» — в текущем коде `linker.ld` уже есть и `.bss` в нём объявлен. Но `llvm-objcopy -O binary` NOBITS-секцию в конец файла не пишет, а `allocate_pages` считает страницы от размера файла, так что `.bss` всё равно не будет ни выделен, ни занулён → issue 003.

## Git

- `*.bin`, `usb_root/`, `OVMF.fd`, `Cargo.lock` — в `.gitignore`. Следствие: `cargo build` в `bootloader/` на чистом клоне падает на `include_bytes!("kernel.bin")`, пока не собраны kernel/app. Порядок в `02_build.sh` это учитывает.
- `Cargo.lock` не коммитится — для бинарных крейтов это снижает воспроизводимость (uefi зафиксирован `=0.27.0`, но транзитивные зависимости плавают) → issue 011.
