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

Main task [750](../issues/750-effector-agent.md): MIND Core gets an agent for the maintainer's Effector fleet server. In the external `sst-test-deploy` project, `issues-done/81-mindcore-server-platform-boundary.task.md.done` records only the prepared local platform/configuration/path boundary; it has not been deployed. Active `issues/81-mindcore-01-agent-and-live-acceptance.task.md` covers deployment and real-agent checks. Signed-package metadata/non-shell entry points remain open in `issues/81-mindcore-02-package-metadata.task.md` and [750-EFF-0008](../issues/750-EFF-0008-application-packages.md). The historical external task 75 now concerns unrelated console work.

- [x] Decide whether the subset of the Effector agent protocol that MIND Core implements may be published in this public repository, as task [750-EFF-0001](../issues/750-EFF-0001-protocol-contract.md) proposes. The subset is the routes, fields and actions the agent uses, without the server's internal weaknesses. **Decided by the maintainer, 2026-10-09:** yes, it may be published; supporting MIND Core is a requirement on the server.
- [ ] For task [750-EFF-0012](../issues/750-EFF-0012-run-against-the-effector-server.md), provide a test Effector server the QEMU host can reach. Name its exact build and the contract reviewed against it; the run needs the deployed platform boundary from external task 81 and the signed-package metadata/non-shell entry points of 81-02 for its package checks. Task 81-01 records the remaining release and live-agent acceptance. Give the session its HTTPS address, its certificate pin verified through the owner's trust channel, a native AgentToken for the test machines and an operator account. Keep credentials out of the repository; an operator API key cannot replace AgentToken.
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

## 9. The Constitution and RFC 001: changes the critique proposes

The critique of 2026-10-14 ([issues-critica-done/01](../issues-critica-done/01_constitution_and_docs_analysis.md)) proposes changes to the founding documents. An agent may not make them on its own: each needs a new edition in English and Russian, with its reason, the guarantees affected, the alternatives and a migration plan ([AGENTS.md](../AGENTS.md), section 1). The kernel session checked each against the texts. Tick what you accept, and the kernel session drafts the edition.

**Constitution 1.7, editorial (no guarantee changes):**

- [ ] **Clause identifiers in the headings.** Write `**MC-2.6.**` in place of `**2.6.**` in Articles 1–12, 114 headings in each language. The preamble already defines `MC-2.6` (line 31), and AGENTS.md cites clauses that way, but a search for `MC-2.6` does not find the clause today.
- [ ] **The links to `../const_discusstion_1.3/`.**
  - Appendix E (lines 483–486) links files that are not in the repository. Make them plain text, as Appendix A already is.
  - Appendix H (line 627) says Appendices A and E keep links; only E does.
- [ ] **Optional: Appendix B.1 and F.1.** B.1 names Untyped pools and retype as the initial model, and F.1 rejects "a physical memory allocator … in the kernel". The kernel keeps a frame pool with per-task quotas. B.1 already allows that if the profile says so (line 354), and 000-KRN-0105 makes the profile say so. A note in B.1 would only spare readers the search.

**Not an amendment.** The critique reads MC-3.6 (a revocation's observable completion) and MC-5.4 (long operations bounded, splittable or preemptible) as a conflict. They are not one:
- MC-3.6 does not ask for a revocation in one step, and MC-5.4 leaves the bound to the profile;
- MC-3.5 already asks for "the fate of operations in progress".

The gap is in the kernel and the profile, and [000-KRN-0104](../issues-done/000-KRN-0104-revocation-bounded-by-the-profile.done) takes it.

**RFC 001 0.5 (Marain; the `MRN` track is open, and the RFC goes through you):**

- [ ] **§11–12, `append`'s example.** It returns `result<_, log-error>`, which cannot say an outcome is unknown, while §7.5 requires that for external effects.
  - §16 and the review (§9, item 4) already plan `OutcomeUnknown` and an `operation_id` for M3–M4.
  - Either bring the example there now, or say in it that the local ring's append is decided before it returns.
- [ ] **§7.5, a counterparty that never answers.**
  - `Uncertain` records count against a quota that limits new operations, and old obligations are not forgotten. If the counterparty is gone for good and cannot be fenced, the actor stops taking new work.
  - The RFC leaves "the reaction to long unavailability" to the profile, and no profile defines it.
  - Proposed: an explicit operation, held only by the lifecycle owner, that closes such a record as abandoned and writes it to the audit log (Article 10). Nothing is forgotten silently.
- [ ] **The header's link** (line 8) to `../const_discusstion_1.3/`: make it plain text.

## 10. A test rig for real machines: keyboard and mouse, the screen, power

The maintainer's offer (2026-10-14): Effector drives a device that emulates a keyboard and a mouse, and a video capture module shows the machine's screen. With them, an agent runs a test on a real machine without a person present ([000-KRN-0108](../issues/000-KRN-0108-tests-on-a-real-machine-through-effector.md)).

- [ ] **Name the parts:**
  - the input device, and whether it is a USB HID boot-protocol keyboard and mouse (firmware menus need that) and whether it has an absolute (tablet) mode;
  - the capture module, its native size, and whether a frame can come lossless;
  - the test machine it is attached to.
- [ ] **Effector operations for them,** on an agent of the rig's host:
  - keys by name with modifiers (QEMU's `sendkey` names), and typing at a set pace;
  - mouse moves, buttons and the wheel;
  - one frame as PNG, and a short clip;
  - their names and fields in docs/effector.md.
- [ ] **Power,** so that a hung machine needs no one: power on, a hard reset and a forced off, through a relay on the front-panel header or a switched socket.
- [ ] **Wanted: the boot medium.** A USB disk the rig writes with the image under test and switches to the machine.
- [ ] **Wanted: the serial line,** where the machine has COM1 or a USB-UART.
- [ ] **Say that this machine may be rebooted and powered off by agents.** The work machines' "no reboots" rule stays as it is.

## 11. Iain M. Banks's names in the project's identity

MIND Core's story borrows from Iain M. Banks's Culture novels: the README's "Why MIND Core", the Minds, the language name Marain (RFC 001), and the repository's own name (`gcu_…`, a General Contact Unit). As a homage in a story this is common. As the core of a brand and a lexicon, which main task [850](../issues/850-the-cultural-code.md) considers, it is a question for a person, and perhaps a lawyer: the names belong to the author's estate.

- [ ] Decide whether the project's public names (Marain, Mind, the ship classes, the Culture) stay, change, or stay only as acknowledged homage, and record the decision in [850](../issues/850-the-cultural-code.md).
- [ ] If they change: RFC 001's name changes in a new revision, in both languages (`constitution/README.md`'s rules).

## 12. A PinePhone to test on

The phone track `MOB` ([TRACKS.md](../TRACKS.md), main task [100000](../issues/100000-a-phone-the-pinephone-first.md)) starts on the original PinePhone, and starts when one is at hand (the maintainer, 2026-10-25). Until then the track records the intention.

- [ ] **A PinePhone,** from Pine64's store: the edition with 3 GB if there is a choice.
- [ ] **Its serial console cable** for the headphone jack (Pine64 sells one), and two or three microSD cards.
- [ ] **Tow-Boot** installed on the phone with its own installer, as Tow-Boot's documentation for the PinePhone says. Send back its version.
- [ ] **The cable connected** to a machine Effector reaches, with the serial port's name, so that an agent reads the phone's boot log there ([docs/effector.md](../docs/effector.md)).

Done when the phone answers on the serial console at 115200 with Tow-Boot's prompt, and the machine and the port are recorded in 100000.

## 13. Network boot off in the firmware of the machines that boot MIND Core

Task [351-KRN-0195](../issues-done/351-KRN-0195-no-firmware-network-boot-before-the-loader.done): the firmware's own network boot (PXE, HTTP boot) runs before our bootloader and outside every guarantee ([threat model](../docs/profile/threat-model.md)). `efivar` refuses to put such an entry first or next, but only the firmware's setup turns it off. The agents cannot reach the firmware's setup screens.

- [ ] On each machine that boots MIND Core (the Intel PC, the MacBook Pro if its firmware offers it, and any Effector machine that boots a MIND Core disk): in the firmware's setup, turn off PXE or "Network Stack", "UEFI Network" and HTTP boot. Note the machine, the firmware's version and the setting's name here.

Done when every such machine is listed with its setting off.
