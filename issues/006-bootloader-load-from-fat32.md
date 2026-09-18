# 006 — Загрузчик: читать ядро и приложение с FAT32 вместо `include_bytes!`

**Тип:** feature · **Приоритет:** средний · **Статус:** open
**Затрагивает:** `bootloader/src/main.rs`, `02_build.sh`, `usb_root/`

## Расхождение с handoff

Handoff: «считывает файлы ядра и приложения из файловой системы FAT32». Код: `include_bytes!("kernel.bin")` / `include_bytes!("app.bin")` (`bootloader/src/main.rs:20-21`); всё вшито в `BOOTX64.EFI` (16 896 байт). На диске в `usb_root/EFI/BOOT/` лежит только EFI.

Последствия текущей схемы: любое изменение ядра требует пересборки загрузчика; `cargo build` в `bootloader/` на чистом клоне не собирается, пока нет `.bin` (они в `.gitignore`).

## План

1. В bootloader: `boot_services.get_image_file_system(image_handle)` → `open_volume()` → `open("\\EFI\\MIND\\kernel.elf", READ)` → `RegularFile` → `get_info::<FileInfo>()` для размера → `allocate_pages` → `read`. Аналогично для `app.elf`.
2. `02_build.sh`: копировать артефакты в `usb_root/EFI/MIND/` (или в корень), не в `bootloader/src/`.
3. Ошибку «файл не найден» печатать через `stdout()` до `exit_boot_services` и делать `stall` + `reset`, а не `unwrap` в `loop {}`.
4. Опционально: путь к файлам через `LoadOptions` или конфиг `EFI\MIND\boot.cfg`.

## Критерии готовности

- `BOOTX64.EFI` не содержит `include_bytes!`; размер EFI не зависит от ядра.
- Замена `kernel.elf` на диске без пересборки загрузчика меняет поведение в QEMU.
- Отсутствие файла даёт читаемое сообщение на экране.

## Связано

Естественно делать вместе с [002](002-elf-loader.md): читать сразу ELF.
