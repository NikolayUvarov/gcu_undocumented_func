# Анализ исходного кода, сборочных скриптов и соответствия Конституции

## 1. Сводная оценка соответствия кода Конституции v1.6

Архитектура системы в ключевых механизмах следует требованиям Конституции v1.6 (`MC-1.1`–`MC-12.9`)[cite: 2, 3]:
* **Минимальное ядро и вынос политик (`MC-1.1`, `MC-1.3`, `MC-1.4`, `MC-3.12`):** Ядро не содержит списка сервисов и запускает только `init`, передавая ему начальные полномочия (`platform` и `spawn`), причём `init` сбрасывает привилегию `platform` до перехода в состояние `READY` и держит делегируемые привилегии в режиме escrow[cite: 2, 3].
* **Отсутствие ambient authority и защита handles (`MC-3.1`, `MC-3.2`, `MC-3.4`, `MC-3.6`):** Внедрены 64-битные дескрипторы с 32-битным поколением, дерево деривации (CDT), сужение прав через `CAP_MINT` (включая 16-битные badges) и отзыв `CAP_REVOKE` с принудительным снятием отображений памяти[cite: 2, 3].
* **Граница Babel / Airlock (`MC-11.4`, `MC-11.9`, `MC-11.11`):** Сетевой путь разделён на `virtio_net`, `netstack`, `netpolicy`, изолированный парсер `parse`, `tls` и `keystore` (где секретный ключ не покидает сервис, а количество подписей ограничено квотой 4096 за загрузку)[cite: 2, 3].
* **Формальная верификация (`MC-12.2`):** В CI (`.github/workflows/ci.yml`) интегрирована проверка TLA+ моделей (`scripts/model_check.sh`) для операций `revoke` и `MOVE`[cite: 2, 3].

Аудит предоставленных исходных файлов (`02_build.sh`, `03_run_qemu*.sh`, `ahci`, `ata`, `audio_gw`, `app`, `app2`) выявил ряд технических проблем и отступлений от требований Конституции[cite: 2, 3].

---

## 2. Проблемы в коде драйверов (Ring 3) и нарушения статей Конституции

### 2.1. Драйвер `ata/src/main.rs`
1. **Бесконечные циклы ожидания без таймаута (`MC-5.4`, `MC-6.1`):**
   * В методах `Ata::command` (`while self.io.in8(COMMAND) & BSY != 0 {}`) и `Ata::flush` (`while self.io.in8(COMMAND) & BSY != 0 {}`) опрос регистра состояния выполняется в бесконечном цикле без верхнего предела итераций или времени[cite: 2].
   * При аппаратном сбое контроллера или залипании бита `BSY` драйвер зависнет навсегда, нарушая `MC-6.1` (локализация зависаний и сбоев устройств)[cite: 2, 3].
2. **Зависимость таймаутов от частоты CPU вместо монотонных часов (`MC-5.6`):**
   * В методах `Ata::probe`, `Ata::wait_ready` (`for _ in 0..10_000_000`) и `Ata::wait_data` (`for _ in 0..1_000_000`) время ожидания измеряется количеством циклов чтения порта ввода-вывода, а не монотонными часами ядра (`mind::time`), что противоречит `MC-5.6`[cite: 2, 3].
3. **Отсутствие валидации размера буфера и аргумента `count` (`MC-2.4`, `MC-2.11`):**
   * В `Ata::command` аргумент `count: usize` усекается до `count as u8`[cite: 2]. Если `count == 0` или не соответствует длине среза `out` / `data` в `read` и `write`, цикл `out.chunks_exact_mut(512).take(count)` выполнит меньше итераций чтения/записи `DATA`, чем ожидает контроллер IDE, оставив устройство в несогласованном состоянии DRQ[cite: 2].

### 2.2. Драйвер `ahci/src/main.rs`
1. **Уязвимость к панике при выходе за границы DMA-буфера (`MC-2.4`, `MC-2.11`, `MC-6.1`):**
   * В `Ahci::write` выполняется `let bytes = count * 512; self.dma.bytes(DATA, bytes).copy_from_slice(&data[..bytes]);`, а в `Ahci::read` — `self.command(0x25, lba, count, &mut out[..count * 512])`[cite: 2].
   * Смещение `DATA` равно `0x10000` (64 КиБ) внутри DMA-региона размером 128 КиБ (остаётся ровно 64 КиБ, или 128 секторов)[cite: 2]. В методах `read` и `write` отсутствует явная проверка `count == 0 || count > 128 || data.len() < bytes`: при нарушении этих условий произойдёт `panic!` на индексации среза, аварийно завершающий процесс драйвера вместо возврата `false`[cite: 2].
2. **Игнорирование результата BIOS/OS Handoff (`MC-1.5`, `MC-6.3`):**
   * В `Ahci::probe` при захвате контроллера у прошивки (`if hba.read32(CAP2) & 1 != 0`) вызывается `wait(|| hba.read32(BOHC) & 1 == 0);`, но возвращаемый `bool` игнорируется[cite: 2]. Если прошивка не отдала контроллер за отведённое время, драйвер начинает программировать DMA при активном владении со стороны BIOS/SMM, что нарушает `MC-1.5` и `MC-6.3`[cite: 2, 3].
3. **Грубая гранулярность ожидания в `wait()` (`MC-5.6`):**
   * Функция `wait()` сначала делает `1_000` холостых прокруток `core::hint::spin_loop()`, а затем переходит на `mind::time::sleep(10)` (шаг 10 мс, хотя в системе на x86 уже есть тик 1 мс и монотонное время `CLOCK`)[cite: 2].

### 2.3. Драйвер `audio_gw/src/hda.rs`
1. **Зашитые в код драйвера платформенные таблицы и константы Apple/Cirrus (`MC-1.3`, поддерживаемость):**
   * В функциях `Hda::apple_amplifiers` и `Hda::cirrus_digital_microphone` жёстко зашиты идентификаторы вендоров и подсистем (`0x1013_4206`, `0x1013_4207`, `0x106B_2800`) и специфичные GPIO/коэффициенты для ноутбуков MacBook Pro[cite: 2].
   * В то время как для `gpio` конфигурации выводов вынесены в декларативные таблицы `hwdocs/` (`bcm2711.pins`, `rpi4b.board`)[cite: 2], в `hda.rs` платформенные quirks смешивают общую логику спецификации Intel HDA с конкретными моделями плат[cite: 2]. По мере расширения парка поддерживаемых ПК (трек `PRT`) такие настройки целесообразно вынести в отдельный модуль quirks или таблицу в `hwdocs/`[cite: 2].
2. **Отсутствие контроля переполнения очереди команд CORB и использование `unwrap()`:**
   * В `Hda::verb` при записи команды в кольцо CORB вычисляется `write = ((self.regs.read16(CORBWP) as usize & 0xFF) + 1) % self.corb_entries`, но не проверяется, не равен ли новый `write` текущему `CORBRP` (если кодек завис)[cite: 2].
   * В `Hda::configure` используется `list.iter().find(|w| w.nid == nid).unwrap()`, который лучше заменить на безопасный обход с `?` или `if let Some(&w)`, исключая точки паники в системном сервисе[cite: 2].

---

## 3. Проблемы в пользовательских приложениях (`app`, `app2`)

1. **Использование устаревших и опасных конструкций `static mut` в `app/src/main.rs`:**
   * В `app/src/main.rs` переменные `MAIN_SP`, `THREAD_SP`, `WORK_COUNTER` и `THREAD_STACK` объявлены как `static mut`[cite: 2]. В современном Rust для `MAIN_SP`, `THREAD_SP` и `WORK_COUNTER` следует использовать `AtomicU64` / `AtomicUsize`, а для стека — обёртку на базе `UnsafeCell`[cite: 2].
   * Функция `background_task()` содержит бесконечный цикл `loop`, но её сигнатура не помечена как расходящаяся (`-> !`), а в `init_thread()` на `x86_64` адрес функции кладётся на стек вручную без возвратного адреса-заглушки[cite: 2].
2. **Дублирование графических примитивов в обход SDK `libmind` (`app/src/main.rs` и `app2/src/main.rs`):**
   * В `libmind` уже реализованы модули `mind::gfx::Screen` и `mind::util::Decimal`[cite: 2]. Однако `app/src/main.rs` по-прежнему реализует собственные `draw_char`, `draw_string`, `usize_to_str` и напрямую пишет в `info.fb_ptr` через `core::ptr::write_volatile`[cite: 2].
   * Аналогично, `app2/src/main.rs` содержит собственные функции `fill_rect` и `draw_text` с сырыми `unsafe` указателями вместо использования `mind::gfx::Screen`[cite: 2].
   * В `app2/src/main.rs` при нажатии `Esc` в лог выводится устаревшая строка `[APP2] ESC PRESSED. RETURNING TO KERNEL.` (фактически управление возвращается пользовательской оболочке `shell`, а не ядру), а при чтении спрайта вызывается избыточный мутабельный метод `sprite.as_mut_slice().chunks_exact(4)` вместо `as_slice()`[cite: 2].

---

## 4. Проблемы в сборочных и стартовых скриптах (`02_build.sh`, `03_run_qemu*`)

1. **Дубликат в массиве `USER_CRATES` в `02_build.sh`:**
   * В файле `02_build.sh` запись `"virtio_input:virtio_input:virtio_input.elf"` присутствует в массиве `USER_CRATES` **дважды** (один раз после `ps2_kbd`, второй раз после `bcm_wifi`)[cite: 2]. На шаге компиляции дубликат отсекается хеш-таблицей `SEEN[$crate_dir]`, но на шаге `[2/2] Staging EFI/ELF files...` скрипт дважды копирует `virtio_input.elf` в `usb_root/`[cite: 2]. Вторую запись необходимо удалить[cite: 2].
2. **Отсутствие поддержки Intel HDA в стартовых скриптах `03_run_qemu*`:**
   * Скрипты `03_run_qemu.sh`, `03_run_qemu_wsl.sh`, `03_run_qemu_windows.bat` и `03_run_qemu_windows_msys2.bat` жёстко задают устройство `-device AC97,audiodev=snd0`[cite: 2]. Несмотря на то что `audio_gw` теперь поддерживает Intel HD Audio (`hda.rs`), ни один из пользовательских скриптов запуска не позволяет переключиться на `-device intel-hda -device hda-duplex,audiodev=snd0` (например, через переменную окружения `MIND_AUDIO_HW=hda|ac97`), а в скриптах для `aarch64` (`03_run_qemu_aarch64.sh`) аудиоустройство вообще не пробрасывается[cite: 2].

---

## 5. Рекомендуемый план действий (по приоритетам)

1. **Скрипты и документация (быстрые исправления):**
   * Удалить дублирующуюся строку `"virtio_input:virtio_input:virtio_input.elf"` из `02_build.sh`[cite: 2].
   * Привести `ROADMAP.md` и `ROADMAP_RU.md` (особенно Раздел 2 и заголовок версии) в соответствие с фактическим состоянием системы[cite: 2].
   * Исправить таблицу системных вызовов (коллизия `#48 TASK_WATCH` / `#48 STAT` и дубликат `#51 STAT`) и устаревшие утверждения про отсутствие NVMe, HDA, x2APIC и Secure Boot в `README.md`[cite: 2].
   * Добавить треки `DEV` и `EFF` в `AGENTS.md`[cite: 2].
2. **Укрепление драйверов (`ata`, `ahci`, `audio_gw`):**
   * В `ahci/src/main.rs` добавить строгую проверку `count <= 128` и длин срезов в `read`/`write`, а также проверку успешности ожидания BIOS/OS handoff[cite: 2].
   * В `ata/src/main.rs` заменить бесконечные циклы `while self.io.in8(COMMAND) & BSY != 0 {}` на ограниченное по монотонному времени ожидание[cite: 2, 3].
   * Вынести аппаратные quirks Apple/Cirrus из `audio_gw/src/hda.rs` в отдельный модуль или таблицу[cite: 2].
3. **Поправки к следующей редакции Конституции (v1.7) и RFC 001 (v0.5):**
   * Добавить префикс `MC-` непосредственно в заголовки всех пунктов статей 1–12 и убрать битые гиперссылки на `../const_discusstion_1.3/`[cite: 3].
   * Уточнить протокол вытесняемого отзыва (`MC-3.6` + `MC-5.4`) и механизм административного освобождения квоты `Uncertain`-операций при безвозвратной гибели внешнего узла (RFC 001 §7.5)[cite: 3].

---

## Review (2026-10-14, the kernel session)

Every claim was checked against `main` dfc42ee, including who calls the code. A crash scenario is real only if a caller can reach it, and here one layer decides several of them: `block_protocol::span` (`libmind/src/block_protocol.rs`) holds every block request to 1–128 sectors (`BLOCK_MAX_SECTORS`) and a buffer of exactly that size, and `block::serve` is the drivers' only caller.

| § | Claim | Verdict | What the code shows | Where it went |
|---|---|---|---|---|
| 1 | The summary of what follows the Constitution | true | — | — |
| 2.1.1 | `ata`: BSY waits without a bound | true | `command` :46, `flush` :89 | [requests-DRV.md](../issues/requests-DRV.md) |
| 2.1.2 | `ata`: waits counted in port reads, not time | true | :26, :38, :55; `monotonic_ns` is available, and `nvme` and `usb_host` already wait to a deadline | [requests-DRV.md](../issues/requests-DRV.md) |
| 2.1.3 | `ata`: `count as u8` and a short transfer | partly true | The pattern is there, but `span` keeps `count` in 1–128 with a matching buffer, so no request reaches it. Worth a check of its own anyway; the comment "up to 256 sectors" is stale | [requests-DRV.md](../issues/requests-DRV.md) |
| 2.2.1 | `ahci`: a panic past the DMA buffer | partly true | It cannot happen today: 128 sectors fill the 64 KiB after `DATA` exactly. It is a hidden coupling between `BLOCK_MAX_SECTORS` and `init`'s `AHCI_DMA_BYTES`; changing either would panic | [requests-DRV.md](../issues/requests-DRV.md) |
| 2.2.2 | `ahci`: the handoff's result ignored | true | :32; the wait for BOHC.BB is missing too | [requests-DRV.md](../issues/requests-DRV.md) |
| 2.2.3 | `ahci`: 10 ms steps where x86 has a 1 ms tick | true | It costs time, not correctness | [requests-DRV.md](../issues/requests-DRV.md) |
| 2.3.1 | `hda.rs`: Apple's and Cirrus's IDs and coefficients inline | true; the remedy no | `hwdocs/` holds facts the hardware cannot report, and these are keyed by IDs it does report. A const table in the driver fits | [551-DRV-0010](../issues/551-DRV-0010-hda-controller-and-codecs.md) |
| 2.3.2 | `hda.rs`: CORB overflow | partly true | `verb` waits for its response, so one verb is in flight; overflow needs 255 stalled verbs in a row. The real fault is near it: a response arriving after a timeout is taken for the next verb's, and every later answer is shifted | [551-DRV-0010](../issues/551-DRV-0010-hda-controller-and-codecs.md) |
| 2.3.3 | `hda.rs`: `unwrap()` in `configure` | false as a crash | The path holds only nodes found in the list, so it cannot be None. It can still go, by keeping the index | [551-DRV-0010](../issues/551-DRV-0010-hda-controller-and-codecs.md) |
| 3.1 | `app`: `static mut` | true | Accessed only through raw pointers with volatile reads and writes, so no fault today | [requests-APP.md](../issues/requests-APP.md) |
| 3.1 | `app`: `background_task` not `-> !`; the stack without a return slot | partly true | `-> !` is missing. The stack claim is false: :47–48 leave the slot, and alignment is as SysV asks | [requests-APP.md](../issues/requests-APP.md) |
| 3.2 | `app`, `app2`: their own drawing in place of `mind::gfx::Screen` and `Decimal` | true | `app2` already uses `Decimal` | [requests-APP.md](../issues/requests-APP.md) |
| 3.2 | `app2`: "RETURNING TO KERNEL"; `as_mut_slice` | true | `dzen-clock` has the same words, and a test checks them | [requests-APP.md](../issues/requests-APP.md) |
| 4.1 | `02_build.sh`: `virtio_input` twice | true | :69 and :92; the staging loops copy it twice | [000-KRN-0107](../issues/000-KRN-0107-a-build-entry-and-comments-that-went-stale.md) |
| 4.2 | The run scripts give QEMU an AC97 only; aarch64 has no audio device | true for x86; the aarch64 part no | `audio_gw` is built for x86 only, so a device on aarch64 would do nothing | [551-DRV-0010](../issues/551-DRV-0010-hda-controller-and-codecs.md) |
