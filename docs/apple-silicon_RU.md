# MIND Core на компьютерах Mac с Apple Silicon

**Версия:** 1.0 (2026-10-08) · **Трек:** `APL`, Apple Silicon ([TRACKS.md](../TRACKS.md)) · **Задачи:** [600](../issues/600-apple-silicon-mac-vm-host.md) (виртуальная машина на Mac), [210](../issues/210-apple-silicon-native.md) (без виртуальной машины) · **Дорожная карта:** [трек H](../ROADMAP_RU.md) · **Конституция:** [v1.6](../constitution/RU/MIND_CORE_Constitution_v1.6.md) MC-12.1, MC-12.3, MC-12.9 · **Английская версия:** [apple-silicon.md](apple-silicon.md) (поддерживается синхронно; эталон — английский текст)

> **Ничего из этого руководства ещё не запускалось на Mac.** CI и все локальные запуски этого репозитория идут на Linux. Шаги следуют из того, что делают скрипты, и из того, как Homebrew и QEMU, по ожиданиям, ведут себя в macOS. У каждого шага сказано, что известно, а что **ещё не проверено на Mac**. Кто выполнит их первым, сообщите, пожалуйста, результат: задача [600-APL-0011](../issues/600-APL-0011-first-run-on-a-mac.md) и [issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on).

## Коротко

| Способ запуска | Состояние |
|---|---|
| Сборка aarch64 в QEMU с гипервизором Apple (HVF) | `03_run_qemu_aarch64.sh` на Mac с Apple Silicon выбирает HVF. **Ещё не проверено на Mac.** Для сборки там нужны Bash и GNU sed из Homebrew (раздел 2). |
| Сборка aarch64 в QEMU с эмуляцией (TCG) | Конфигурация, которую проверяет CI, на Linux (`-cpu max`). На Mac так идут тестовые наборы (раздел 5); **ещё не проверено на Mac**. |
| Без виртуальной машины | **Пока не поддерживается** (раздел 7); задача [210](../issues/210-apple-silicon-native.md), трек `APL`. |
| Сборка x86-64 | На Mac QEMU может машину x86 только эмулировать; `03_run_qemu.sh` написан для Linux. В этом руководстве не рассматривается. |

То, что утверждает профиль [`aarch64/QEMU-virt-0`](profile/aarch64/README.md), проверено на QEMU `virt` с TCG на Linux, с `-cpu max`. На Mac с HVF и `-cpu host` это не переносится (MC-12.1, MC-12.9): это другая конфигурация, и в ней ничего не проверено (раздел 4).

## 1. Что нужно

- Mac с Apple Silicon (M1 или новее) и [Homebrew](https://brew.sh), который на таких Mac живёт в `/opt/homebrew`. Homebrew ставит Xcode Command Line Tools; сборка компонует с ними свои вспомогательные программы для хоста (сценарии сборки и процедурные макросы крейтов).
- Из Homebrew:

  ```bash
  brew install qemu bash gnu-sed
  ```

  - `qemu`: `qemu-system-aarch64`, `qemu-img` и прошивка UEFI EDK2 для aarch64 (`share/qemu/edk2-aarch64-code.fd` и `share/qemu/edk2-arm-vars.fd` внутри `$(brew --prefix)`).
  - `bash`: сценарию сборки aarch64 нужен Bash 4.4 или новее; `/bin/bash` в macOS — версии 3.2.
  - `gnu-sed`: сценарий сборки aarch64 использует форму `sed`, которую `sed` из macOS, по ожиданиям, отвергнет.
- Rust через rustup, с закреплённым nightly и целями из `rust-toolchain.toml` (среди них `aarch64-unknown-none-softfloat` и `aarch64-unknown-uefi`):

  ```bash
  ./01_prepare_env.sh
  source "$HOME/.cargo/env"
  ```

  Это цели для «голого железа» и UEFI, поэтому rustup, по ожиданиям, даёт их на хосте с macOS так же, как на Linux, а программы компонуются через `rust-lld` из состава toolchain, а не системным компоновщиком. Если установлен ещё и `rust` из Homebrew, `command -v cargo` должен печатать `~/.cargo/bin/cargo`. **Ещё не проверено на Mac.**
- Python 3.9 или новее для USB-образа и тестов (свой `python3` macOS или из Homebrew).

## 2. Сборка на Mac

```bash
PATH="$(brew --prefix gnu-sed)/libexec/gnubin:$PATH" "$(brew --prefix)/bin/bash" scripts/build_aarch64.sh
```

Это то, что на Linux запускает `ARCH=aarch64 ./02_build.sh`. Команда собирает загрузчик (`aarch64_root/EFI/BOOT/BOOTAA64.EFI`), ядро и все программы для aarch64 в `aarch64_root/`. С `--fixtures` собираются и тестовые программы (раздел 5). **Ещё не проверено на Mac.**

Почему не `ARCH=aarch64 ./02_build.sh`, как на Linux:

- `02_build.sh` запускает `scripts/build_aarch64.sh` через его первую строку `#!/bin/bash`; в macOS это Bash 3.2, в котором нет `mapfile`, поэтому сценарий, по ожиданиям, сразу остановится (`mapfile: command not found`).
- С более новым Bash, но с `sed` из macOS сценарий, по ожиданиям, не прочитает из `02_build.sh` ни одной программы и остановится с `USER_CRATES not found in 02_build.sh`.

Команда выше запускает сценарий в Bash из Homebrew и ставит GNU sed первым в `PATH`. Сделать так, чтобы сборка работала с тем, что есть в самой macOS, — задача [600-APL-0009](../issues/600-APL-0009-aarch64-build-on-macos.md).

## 3. Запуск в виртуальной машине с гипервизором

```bash
./03_run_qemu_aarch64.sh -display cocoa
```

**Ещё не проверено на Mac.** Что делает сценарий на Mac с Apple Silicon (он проверяет, что `uname -s` — `Darwin`, а `uname -m` — `arm64`):

| | На Mac с Apple Silicon | На Linux |
|---|---|---|
| Ускоритель и процессор | `-accel hvf -cpu host`: гостевая система работает на собственных ядрах Mac | `-cpu max`, эмуляция (TCG) |
| Машина | `virt,gic-version=3,highmem=off`, 512 МиБ, 4 процессора | так же |
| Прошивка | `edk2-aarch64-code.fd` и личная копия `edk2-arm-vars.fd` из `share/qemu` рядом с запускаемым QEMU, для QEMU из Homebrew — `$(brew --prefix)/share/qemu` | AAVMF (Debian, Ubuntu), edk2 (Fedora) или прошивка самого QEMU |
| Диск | `aarch64_root/` как виртуальный диск FAT | так же |
| Устройства | экран `ramfb`, сетевая карта VirtIO (пользовательская сеть QEMU), клавиатура и планшет VirtIO | так же |
| Консоль | PL011 в этом терминале (`-serial mon:stdio`); **Ctrl+A X** — выход из QEMU | так же |

- **Окно.** Сценарий открывает окно, только если задана `DISPLAY` или `WAYLAND_DISPLAY` (рабочий стол Linux, WSLg). В macOS обычно не задана ни одна, поэтому он передаёт `-display none`, и остаётся только консоль в терминале. `-display cocoa` после этого просит у QEMU его окно macOS: QEMU, по ожиданиям, берёт последний переданный `-display`. В окне клавиши идут на клавиатуру VirtIO, указатель — на планшет VirtIO, так что QEMU не нужно захватывать указатель. Задача [600-APL-0010](../issues/600-APL-0010-run-script-on-macos.md) научит сценарий открывать окно на Mac самому.
- **Родной терминал.** Запускайте сценарий из терминала, работающего нативно (arm64). Под Rosetta `uname -m` печатает `x86_64`, и сценарий откатывается на эмуляцию (TCG).
- **Bash.** Первая строка сценария — `#!/usr/bin/env bash`: он запускает первый `bash` в `PATH`. Это Bash из Homebrew, когда каталоги Homebrew стоят первыми, как их ставит настройка оболочки Homebrew (`brew shellenv`). Под Bash 3.2 из macOS `MIND_NET=none` или `DISPLAY`, заданная XQuartz, по ожиданиям, остановят сценарий с `unbound variable` (пустой массив при `set -u`). Задача 600-APL-0010.
- **Прошивка.** Если сценарий пишет `No aarch64 UEFI firmware`, укажите файлы:

  ```bash
  MIND_AAVMF_CODE="$(brew --prefix)/share/qemu/edk2-aarch64-code.fd" \
  MIND_AAVMF_VARS="$(brew --prefix)/share/qemu/edk2-arm-vars.fd" ./03_run_qemu_aarch64.sh -display cocoa
  ```

- **Процессоры.** `MIND_CPUS=n` (по умолчанию 4). Каждый виртуальный процессор — поток QEMU, который macOS выполняет на любом из ядер Mac; у M1 их 8: четыре производительных и четыре энергоэффективных. Держите `n` не больше `sysctl -n hw.ncpu`. Ядро запускает все процессоры, перечисленные в таблицах ACPI прошивки; проверено до 16 — с TCG на Linux, с HVF — ни одного.
- **Память.** `MIND_MEMORY=размер` (по умолчанию `512M`). До `3G` машина держит ОЗУ и PCI ниже 4 ГиБ (`highmem=off`, раскладка, которую проверяют тестовые наборы). Выше `3G` сценарий ставит `highmem=on`. С HVF M1, по ожиданиям, даёт гостю 36-битное физическое адресное пространство (64 ГиБ); помещается ли в него раскладка QEMU, не проверено, поэтому оставайтесь на `3G` или ниже.
- **Остальные настройки** — как на Linux: `MIND_NET=none` убирает сетевую карту, `MIND_DISPLAY=none` — окно, `QEMU=<программа>` указывает другой QEMU; прочие аргументы передаются QEMU.

### Что показывает удачная загрузка

В терминале: строки прошивки, затем строки ядра, среди них `MIND CORE KERNEL: BOARD GICV3 … ITS=… … CPUS n` с раскладкой, прочитанной из ACPI, строки служб, `[INIT] READY` и приглашение `MIND>`. В окне — экран системы. `reboot` перезапускает машину, `reboot --off` её выключает (PSCI). Сообщите, пожалуйста, что вы видите, в [600-APL-0011](../issues/600-APL-0011-first-run-on-a-mac.md), вместе с:

- строкой `MIND CORE KERNEL: BOARD …` (дал ли QEMU ITS);
- `[KEYSTORE] DEVICE KEY READY …` или `[KEYSTORE] NO RNDR: NO DEVICE KEY` (раздел 4);
- моделью Mac и его чипом, выводом `sw_vers` и `qemu-system-aarch64 --version`.

### USB-образ в виртуальной машине

```bash
./04_make_usb_image_aarch64.sh --no-build    # после сборки из раздела 2
./03_run_qemu_aarch64.sh --image -display cocoa
```

`04_make_usb_image_aarch64.sh` сначала собирает систему, запуская `02_build.sh`, а тот в macOS останавливается, как сказано в разделе 2; отсюда сборка по разделу 2 и `--no-build`. Сценарий упаковывает `aarch64_root/` в `dist/mind-core-usb-aarch64.img` с помощью Python и `qemu-img` (входит в `qemu` из Homebrew). `--image` загружает этот файл как USB-флешку на xHCI; QEMU ничего в него не пишет. **Ещё не проверено на Mac.** Образ предназначен для плат с прошивкой UEFI (задача [205](../issues/205-aarch64-boards.md)); `05_write_usb_linux.sh`, записывающий его на флешку, работает только в Linux. Mac загрузить его не может (раздел 7).

## 4. Чем это отличается от проверенной конфигурации

- **Процессор (`-cpu host`).** Гость видит возможности ядер Mac вместо модели `max` из QEMU. Служба ключей делает ключ устройства из RNDR — инструкции случайных чисел; есть ли она у ядер Apple, здесь не проверялось. Без неё система работает, но служба ключей не создаёт ключа устройства, а служба TLS отказывает в любом соединении (`[KEYSTORE] NO RNDR: NO DEVICE KEY`) — так набор `tls` показывает для Cortex-A72 с TCG. С TCG (`-cpu max`, раздел 5) RNDR есть.
- **Регистры устройств.** С HVF QEMU эмулирует обращение к регистру устройства, только если процессор полностью описывает это обращение гипервизору: одна загрузка или запись одного регистра, без пары и без обратной записи базового регистра. Другие формы, по ожиданиям, остановят QEMU с внутренней ошибкой. TCG эмулирует любую инструкцию, поэтому CI не может показать, пользуются ли ядро или драйверы такими формами в памяти устройств.
- **Прерывания.** GICv3 — собственная модель QEMU, как и с TCG. Даёт ли QEMU с HVF ITS (MSI-X), не проверялось; без него ядро оставляет драйверам их выделенные линии, как профиль говорит о `virtio_net` (`ITS=0x0` в строке `BOARD`).
- **Время.** Общий таймер считает с частотой Mac. Ядро читает частоту из `CNTFRQ_EL0`, так что разницы, по ожиданиям, не будет.
- **Скорость.** Виртуальные процессоры делят ядра Mac с macOS. Времена отличаются от TCG на Linux, где задавались пределы времени тестовых наборов.

## 5. Тесты на Mac

Наборы для QEMU запускают QEMU сами, с `-cpu max` и без ускорителя, поэтому на Mac они идут с TCG, как в CI, и ничего не проверяют в HVF. Прошивку они ищут там, куда её кладёт Debian; укажите прошивку из Homebrew:

```bash
PATH="$(brew --prefix gnu-sed)/libexec/gnubin:$PATH" "$(brew --prefix)/bin/bash" scripts/build_aarch64.sh --fixtures
FW="$(brew --prefix)/share/qemu"
python3 tests/aarch64_smoke.py --code "$FW/edk2-aarch64-code.fd" --vars "$FW/edk2-arm-vars.fd"
python3 tests/qemu_smoke.py --arch aarch64 --aavmf-code "$FW/edk2-aarch64-code.fd" --aavmf-vars "$FW/edk2-arm-vars.fd"
```

**Ещё не проверено на Mac.** Чем, по ожиданиям, будет отличие от Linux:

- Наборы `normal` и `smp` измеряют процессорное время QEMU через `/proc/<pid>/stat`, которого в macOS нет, поэтому на этой проверке они, по ожиданиям, упадут. `--suites shell,vfs,store,net,tls,busy` их исключает.
- Набору `vfs` нужны `mkfs.fat`, `fsck.fat` и mtools (`brew install dosfstools mtools`); без них он сообщает SKIP.
- Набор `tls` запускает `openssl` для своих сертификатов и тестовых серверов, с параметрами OpenSSL. Свой `openssl` в macOS — это LibreSSL, параметры которого могут отличаться: установите `openssl@3` из Homebrew и поставьте `"$(brew --prefix openssl@3)/bin"` первым в `PATH`.
- Тесты на хосте (`rustc --test tests/*_host.rs`, список CI) собираются для самого Mac; не пробовались.

Сделать так, чтобы наборы шли на Mac, с TCG и с HVF, — задача [600-APL-0012](../issues/600-APL-0012-aarch64-suites-on-a-mac.md).

## 6. UTM

[UTM](https://mac.getutm.app), оболочка QEMU для macOS, запускает QEMU с тем же гипервизором, поэтому машину из раздела 3 в нём в принципе можно настроить: машина ARM64 `virt` с загрузкой UEFI, экран `ramfb` и USB-образ из раздела 3 в качестве диска. Шагов для UTM это руководство не даёт: никто их не пробовал. Если они сработают, задача 600-APL-0011 добавит их сюда.

## 7. Без виртуальной машины: пока не поддерживается

MIND Core не может работать на Mac с Apple Silicon без виртуальной машины. Ставить на Mac для этого пока нечего. Причины — те, что в задаче [210](../issues/210-apple-silicon-native.md); её подзадачи перечислены в разделе 8. Сведения об оборудовании ниже взяты из документации проекта Asahi Linux; здесь ни одно из них на Mac не проверялось.

- **Нет UEFI.** Mac загружает iBoot от Apple, а `BOOTAA64.EFI` — приложение UEFI. План — загружаться так, как Asahi Linux: его загрузчик m1n1 запускает U-Boot, а тот даёт среду UEFI. Установка их установщиком Asahi означает один раз понизить безопасность загрузки для новой загрузочной записи в recoveryOS; macOS остаётся. Задача 210-APL-0001. Позже свой загрузчик может заменить U-Boot после m1n1 (задача 210-APL-0013), а затем и сам m1n1 — как «сырой» образ, который iBoot запускает, когда это разрешено через `kmutil` (задача 210-APL-0014). Цена второго шага — настройка ядер и питания, которую делает m1n1, и чтение NVMe для обновлений без recoveryOS.
- **Нет ACPI.** Ядро читает раскладку машины из таблиц ACPI (MADT, SPCR, GTDT, MCFG, FADT). Mac вместо этого описан деревом устройств. Задача 210-APL-0002.
- **Не GIC.** Контроллер прерываний — AIC от Apple (AIC2 на M1 Pro, Max и Ultra и на более поздних чипах), а прерывание таймера приходит как FIQ, которого ядро не принимает. Задача 210-APL-0003.
- **Нет PSCI.** Остальные процессоры запускаются через spin table, а не через PSCI `CPU_ON` (задача 210-APL-0004); сброс идёт через сторожевой таймер, а не через PSCI `SYSTEM_RESET` (задача 210-APL-0005).
- **Консоль.** UART — в стиле Samsung, а не PL011, и доступен только через порт USB-C, переключённый в отладочный режим. Задача 210-APL-0006.
- **DMA за IOMMU.** Каждое устройство с DMA обращается к памяти только через свой IOMMU (DART), со страницами по 16 КиБ, тогда как у ядра они по 4 КиБ. Задача 210-APL-0007. DART дали бы MIND Core и границу DMA из MC-1.5, которой пока нет ни на одной её платформе.
- **USB.** Порты Type-C — контроллеры Synopsys DWC3 с PHY от Apple (ATC) и доменами питания (PMGR), а не xHCI на PCI, какой `usb_host` находит сегодня. Задача 210-APL-0008.
- **Встроенные устройства.** Встроенные клавиатура и трекпад (SPI или MTP), NVMe (ANS с прошивкой RTKit), Wi-Fi, звук и контроллер дисплея (DCP) — собственные устройства Apple. Они будут позже, отдельными задачами, и написаны заново по документации: драйверы Linux под GPL, и копировать их в MIND Core нельзя.

Первая цель — критерий приёмки задачи 210: Mac mini или MacBook с M1 загружается с USB-флешки до оболочки на своём экране, с внешней USB-клавиатурой.

## 8. Открытые задачи

Трек `APL` открыт: его может взять любой ([TRACKS.md](../TRACKS.md), [AGENTS.md](../AGENTS.md)). Задачи, которым нужен Mac, ждут человека с Mac ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)).

| Задача | Что |
|---|---|
| [600](../issues/600-apple-silicon-mac-vm-host.md) | Mac как хост: система aarch64 в виртуальной машине с HVF, собранная, запущенная и проверенная в macOS |
| [600-APL-0009](../issues/600-APL-0009-aarch64-build-on-macos.md) | Сборка aarch64 с теми Bash и sed, что есть в macOS |
| [600-APL-0010](../issues/600-APL-0010-run-script-on-macos.md) | `03_run_qemu_aarch64.sh` в macOS: экран в окне, Bash 3.2, выбор ускорителя |
| [600-APL-0011](../issues/600-APL-0011-first-run-on-a-mac.md) | Первый запуск на Mac вручную, с записью результата здесь и в профиле |
| [600-APL-0012](../issues/600-APL-0012-aarch64-suites-on-a-mac.md) | Наборы aarch64 на Mac с TCG и HVF; свидетельства для конфигурации с HVF |
| [210](../issues/210-apple-silicon-native.md) | Mac с Apple Silicon без виртуальной машины, сначала M1 |
| [210-APL-0001](../issues/210-APL-0001-boot-through-m1n1-and-u-boot.md) | Загрузка через m1n1 и U-Boot: `BOOTAA64.EFI` из UEFI U-Boot, вход на EL2 |
| [210-APL-0002](../issues/210-APL-0002-board-from-the-device-tree.md) | Плата из дерева устройств там, где нет ACPI |
| [210-APL-0003](../issues/210-APL-0003-aic-and-the-timer-fiq.md) | AIC (AIC2) и FIQ таймера |
| [210-APL-0004](../issues/210-APL-0004-cpus-through-the-spin-table.md) | Остальные процессоры через spin table |
| [210-APL-0005](../issues/210-APL-0005-reset-without-psci.md) | Сброс через сторожевой таймер, без PSCI |
| [210-APL-0006](../issues/210-APL-0006-samsung-style-uart-console.md) | Консоль на UART в стиле Samsung |
| [210-APL-0007](../issues/210-APL-0007-dart-dma-boundary.md) | DART: DMA каждого устройства только через его IOMMU; страницы по 16 КиБ |
| [210-APL-0008](../issues/210-APL-0008-usb-on-type-c-ports.md) | USB на портах Type-C: DWC3 в режиме хоста, PHY ATC, питание PMGR |
| [210-APL-0013](../issues/210-APL-0013-own-stage-two-instead-of-u-boot.md) | Свой второй этап после m1n1, без U-Boot |
| [210-APL-0014](../issues/210-APL-0014-own-first-stage-instead-of-m1n1.md) | Свой первый этап, который запускает iBoot, без m1n1 |

## 9. Что проверено

На Mac — ничего. Когда шаг этого руководства будет выполнен, его пометка «ещё не проверено на Mac» заменится тем, что было запущено, на каком Mac, с какой macOS и каким QEMU, а профиль aarch64 запишет это как отдельную конфигурацию (MC-12.1).
