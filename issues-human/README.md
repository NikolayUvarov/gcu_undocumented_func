# issues-human/ — tasks only a person can do

Things the agents working on this repository cannot do: repository settings on GitHub (agents have no admin access), legal decisions, and coordination between agent sessions. Mark an item done by ticking it and noting the date; delete the file section when it no longer matters.

## 1. Enable private vulnerability reporting

`SECURITY.md` tells reporters to use **Security → Report a vulnerability**; that button exists only when the feature is on.

- [ ] Repository → **Settings** → **Security** (Code security) → **Private vulnerability reporting** → **Enable**.

## 2. Repository description and topics

Makes the project findable on GitHub.

- [ ] Repository main page → gear icon next to **About**:
  - Description: `MIND Core: capability microkernel in Rust for x86-64 UEFI, with ring-3 drivers and services`
  - Topics: `rust`, `microkernel`, `operating-system`, `osdev`, `uefi`, `x86-64`, `capabilities`
  - Keep "Releases" and "Packages" unticked until there is a release.

The repository keeps its name (`gcu_undocumented_func`); the `repository` field of every `Cargo.toml` already points to it.

## 3. Optional: protect `main`

CI (`.github/workflows/ci.yml`) runs on every push. To stop a red build from reaching `main`:

- [ ] **Settings** → **Rules** → **Rulesets** → new branch ruleset for `main`: require status checks `build` and the `QEMU (...)` jobs; block force pushes.

Note: agents currently update `main` by fast-forward pushes after CI is green on their branch; with required checks they keep working the same way, because the checked commit is the one pushed.

## 4. A Mac with Apple Silicon to test on

The Apple Silicon track `APL` ([TRACKS.md](../TRACKS.md)) needs a person with an Apple Silicon Mac: nothing in this repository has run on one, and the agents have none. An M1 first; an M2, M3 or later is welcome too.

- [ ] **The virtual machine** (main task [600](../issues/600-apple-silicon-mac-vm-host.md)): follow [docs/apple-silicon.md](../docs/apple-silicon.md) ([Russian](../docs/apple-silicon_RU.md)) on the Mac and send back the terminal output, the Mac's model and chip, `sw_vers` and `qemu-system-aarch64 --version` (tasks [600-APL-0011](../issues/600-APL-0011-first-run-on-a-mac.md) and [600-APL-0012](../issues/600-APL-0012-aarch64-suites-on-a-mac.md)). Nothing but Homebrew's packages and rustup needs to be installed on the Mac.
- [ ] **Natively** (main task [210](../issues/210-apple-silicon-native.md)), later: a Mac that may get m1n1 and U-Boot through the Asahi installer, which means lowering, once and in recoveryOS, the boot security of the boot entry it adds (a spare Mac is best); and, for the console, a way to reach its UART over USB-C: a second Apple Silicon Mac with Asahi's `macvdmtool`, or a serial adapter made for it (task [210-APL-0006](../issues/210-APL-0006-samsung-style-uart-console.md)).

## 5. An Intel PC and a SATA SSD for the first real x86 boot

Main task [211](../issues/211-intel-pc-from-a-sata-ssd.md): the maintainer has a Samsung 860 PRO and an Intel PC. Nothing in the profile has run on a physical x86 machine yet.

- [ ] Write the image to the SSD as 211 says (a USB-SATA enclosure or adapter for now), boot the PC from it on its first SATA port, and send back what task [211-PRT-0004](../issues/211-PRT-0004-first-run-on-an-intel-pc.md) asks for: the machine, the firmware settings, a photo of the screen, and the serial output if the board has COM1.
- [ ] After the first boot works: try Secure Boot with the project's own keys, as [docs/update/secure-boot.md](../docs/update/secure-boot.md) describes (on a machine you can restore: it removes the vendor's keys). Send back the firmware's menus, whether the signed disk boots and the unsigned one is refused, and what had to differ from the steps (task [351-UPD-0012](../issues/351-UPD-0012-secure-boot-with-our-own-keys.md)).

## 6. The training data behind the English dictation model

Main task [250](../issues/250-voice-dictation.md) (voice V3) chose models measured on the same sentences:

- **Russian:** alphacep's `vosk-model-ru` 0.54. Its weights are under Apache-2.0, and its training data is not published.
- **English:** the k2-fsa Zipformer GigaSpeech 2023-12-12. It runs on the same engine as the Russian model.
  - Its weights are under Apache-2.0.
  - It was trained on GigaSpeech, whose audio SpeechColab licenses for non-commercial research and education only. SpeechColab does not own the audio's copyright.

Whether such terms reach weights trained on the data is a legal question, and the agents do not decide it. MIND Core is MIT OR Apache-2.0, and the model would ship on its boot disk.

- [x] Decide whether MIND Core may ship the GigaSpeech Zipformer. **Decided by the maintainer, 2026-10-08:** any free model may be used and shipped, including models whose terms allow only non-commercial use. Each model's licence and terms are recorded with it in `models/manifest.toml` and travel with it ([251](../issues/251-model-cache-and-model-disk.md)).
- [x] If not, choose one of the alternatives in 250. Not needed; English keeps the GigaSpeech Zipformer as its compact model, and Parakeet TDT 0.6B v3 (CC BY 4.0) is its quality model.
- [x] Optionally, the same question for the Russian model, whose training data is unknown. Covered by the same decision.

## 7. An Effector server for MIND Core

Main task [750](../issues/750-effector-agent.md): MIND Core gets an agent for the maintainer's Effector fleet server. The server is the maintainer's own project, outside this repository; its side of the work is its task 75.

- [x] Decide whether the subset of the Effector agent protocol that MIND Core implements may be published in this public repository, as task [750-EFF-0001](../issues/750-EFF-0001-protocol-contract.md) proposes. The subset is the routes, fields and actions the agent uses, without the server's internal weaknesses. **Decided by the maintainer, 2026-10-09:** yes, it may be published; supporting MIND Core is a requirement on the server.
- [ ] For task [750-EFF-0012](../issues/750-EFF-0012-run-against-the-effector-server.md), provide a test Effector server the QEMU host can reach. Name its build: at least 1.2.0.127, and a build with task 75's part A for the full run. Give the session its HTTPS address, its certificate pin, an agent token for the test machines and an operator account. Keep the token and the account out of the repository.
- [ ] Say whether MIND Core endpoints may be registered on that server, and under which tag, so that they do not mix with the production terminals.

## 8. A boot and health indicator on an RP2040 board

The maintainer has a USB board with an RP2040 and an RGB LED (2026-10-14). Plugged into a machine that runs MIND Core, it is to:
- show on its LED how far the boot has come and how healthy the system is;
- pass the system log on to its own serial pins, so that a second computer can read it on a machine whose screen is black or that has no COM1 (the Ryzen machine of [213](../issues/213-amd-ryzen-pc.md)).

This item is the board's firmware, built and tested on its own, against a script on a PC that plays MIND Core's part. MIND Core's own part is the agents' work afterwards (below).

### What the firmware does

- **USB:**
  - it enumerates as a standard CDC-ACM serial port at full speed, so that no driver beyond the class driver is needed, on MIND Core, Linux, Windows or macOS;
  - its product string is `MIND Core indicator`. MIND Core sends its log only to a device that says so, never to any serial port that happens to be plugged in;
  - for development, Raspberry Pi's RP2040 vendor ID with a product ID it grants (2E8A); a product ID of its own before others use it.
- **The protocol:** lines of UTF-8 text, LF-terminated, from the system to the board.
  - `@STATE <stage>`, where `<stage>` is `KERNEL`, `SERVICES <started>/<all>`, `READY`, `REBOOT` or `OFF`;
  - `@HEALTH OK|WARN|FAIL <reason>`;
  - `@BEAT <uptime ms>`, once a second;
  - any other line is a log line, as `log:bootNNNN.log` holds it.
  - The board sends nothing the system acts on; MIND Core reads nothing from it.
- **The LED:**

  | Colour | Meaning |
  | --- | --- |
  | Off | No power |
  | Dim blue | Powered, nobody talking to it (firmware, bootloader, the first second of the kernel, or another OS) |
  | Yellow | `KERNEL` or `SERVICES`, brighter as more services start |
  | Green | `READY` with `HEALTH OK` |
  | Orange | `HEALTH WARN` |
  | Red | `HEALTH FAIL` |
  | Magenta, blinking | No `@BEAT` for 3 s after one came: the system or its USB stack stopped |
  | White flash | `REBOOT` or `OFF` |

  The same meanings drawn from one table in the firmware, at a brightness that does not dazzle.
- **The serial pins:**
  - every line received goes out of UART0 TX (3.3 V, 115200 8N1);
  - the last 16 KiB are kept in RAM and sent again when its button is pressed, so that the start of a boot is not lost to a reader who plugged in late.
- **The build:**
  - its source in this repository, in a directory the maintainer names (for example `devices/rp2040-indicator/`);
  - built to a UF2 file to flash with BOOTSEL, by one documented command;
  - Rust (`rp2040-hal` or `embassy-rp`, `usb-device`, `usbd-serial`, PIO for a WS2812) or C with the Pico SDK and TinyUSB;
  - every third-party part in [THIRD_PARTY.md](../THIRD_PARTY.md).

### Done when

- [ ] The board's model, its LED (WS2812 on which pin, or three PWM pins) and the chosen language are named.
- [ ] The firmware builds from source by its documented command and flashes as UF2.
- [ ] A PC test script (`--port`) plays a boot:
  - `KERNEL`, `SERVICES 1/30` … `30/30`, `READY`, `HEALTH WARN`, `HEALTH FAIL`, `REBOOT`;
  - log lines;
  - then it stops its beats.
- [ ] Each of the script's states shows its colour. Magenta comes within 3 s after the beats stop.
- [ ] Every line reaches UART0, read by a USB-UART adapter on a second PC, and the button sends the kept 16 KiB again.
- [ ] A page in `docs/` gives the pins, the colours, the protocol and how to flash it, in English and Russian.

### Afterwards, MIND Core's part (agents' tasks, numbered when started)

- **The drivers track:** a CDC-ACM class driver with `usb_host`.
- **The kernel track:** a service that finds the indicator by its product string and sends it:
  - the kernel's lines kept since the boot, and the system log from its start;
  - `@STATE` from `init`'s start of the services;
  - `@HEALTH` from the failures `init` and `logd` see;
  - `@BEAT`.
- **What it cannot cover:** the board shows nothing of the bootloader or of the first second or so, before `usb_host` runs. A kernel panic shows as the beats stopping, not as text, since USB runs in ring 3.
