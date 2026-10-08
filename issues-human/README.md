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
