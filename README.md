# MIND CORE

Iain M. Banks supposed # MIND CORE

The Minds of the Culture's spaceships — the ultra-powerful AIs controlling General Contact Units and Orbitals — must reside somewhere. No matter how multidimensional and advanced their hardware substrate may be, at a fundamental level, any computing architecture needs two basic things to wake up and become self-aware: a bootloader and an operating system.

Before a Mind can simulate pocket universes, juggle hyperspace vectors, or conduct delicate diplomatic games on behalf of an entire civilization, it requires a reliable and predictable foundation. It is time to start building it.

This repository is the first step toward creating a substrate-independent environment for future Minds. We are starting right now, from the very lowest level of hardware reality.

Every Mind needs its first processor tick. We are providing exactly that.


### Goals and Architecture

* **Substrate Initialization (Bootloader):** Direct interaction with UEFI, seizing control of the bare metal, and preparing physical memory.
* **Kernel:** The basic reality dispatcher. Handling hardware interrupts, system calls, and preemptive multitasking.
* **Isolated Environment (Userspace):** A space for the genesis and parallel execution of high-level processes and future cognitive functions.

---

### Step 0: The Flat Binary Foundation (Current State)

We began with a minimalist bare-metal approach, temporarily bypassing complex executable formats (like ELF) to establish a direct, undeniable link between the hardware, the kernel, and the userspace.

* **Toolchain:** Pure Rust (`no_std`, `no_main`), utilizing `naked_functions` and `abi_x86_interrupt`, compiled for `x86_64-unknown-none` and `x86_64-unknown-uefi` targets.
* **Memory Layout:** The kernel and application are compiled as flat binaries. We use `llvm-objcopy` to strip headers and extract pure executable logic (`.text`, `.rodata`, `.data`), loading them directly into physical memory.
* **Event Model:** Hardware interrupts (IDT) for timing and keyboard input, combined with software interrupts (`int 0x80`) for system calls.

---

### How to Build and Run

**Prerequisites:**
You will need a nightly Rust toolchain, LLVM tools for binary extraction, and a virtual machine capable of UEFI execution.

```bash
rustup toolchain install nightly
rustup default nightly
rustup target add x86_64-unknown-none x86_64-unknown-uefi
rustup component add llvm-tools-preview

```

*Note: Ensure QEMU and the OVMF firmware (UEFI for QEMU) are installed on your host system.*

**Build & Execution:**
The compilation and packaging pipeline is automated via bash scripts.

1. Ensure your build script is executable:
```bash
chmod +x patch_008_preemptive.sh

```


2. Run the script to compile the Kernel and App, extract the flat binaries, and build the UEFI Bootloader into the `usb_root/EFI/BOOT/` directory:
```bash
./patch_008_preemptive.sh

```


3. Launch the environment in QEMU, pointing it to your UEFI firmware and the generated root directory:
```bash
qemu-system-x86_64 \
  -bios /usr/share/ovmf/OVMF.fd \
  -drive format=raw,file=fat:rw:usb_root

```



*(Adjust the path to `OVMF.fd` depending on your OS and package manager).*