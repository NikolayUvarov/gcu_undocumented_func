# 264 — Ladybird on MIND Core: a port kept outside the OS's own code

**Type:** main task · **Owner:** the track `LBD` (opened 2026-11-27, **later**: no session yet; written by `APP`) · **Priority:** the maintainer's to set · **Status:** later (2026-11-27): it starts when the prerequisites below are in `main` · **Blocked by:** `KRN` (images over 8 MiB, asked; TLS, threads and an application's own helpers, to be asked), `NET` (a grant for browsing, to be asked), `DEV` ([650](650-building-on-the-target.md) stages 2 and 3: a C/C++ cross toolchain and a POSIX layer in Rust; the track is open, with no owner), `UPD` (a package of this size, to be checked) · **Roadmap:** track G · **Constitution:** MC-3.1, MC-3.3, MC-11.1, MC-11.8, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-11-27, at the maintainer's question «можно ли портировать https://github.com/ladybirdbrowser/ladybird в нашу ОС», and his words on how: «создай план портирования в наиболее коротком и простом виде так, чтобы не загрязнять основную ОС другими языками». Our own browser stays a development of its own ([256](256-graphical-web-browser-current-standards-only.md), [255](255-text-web-browser.md)). The maintainer then: «хорошо создай план. Порт ladybird выделим в отдельный трек и начнем, когда вся подготовительная работа в других треках это позволит начать».

## What was measured (2026-11-27)

- **Ladybird's source,** `master` at f6c1af9e, read and not built:
  - **Its own code is 1.9 million lines.** LibWeb is 975 thousand of them, LibJS 382, AK 48, the services 92 and the Qt UI 68.
  - **The toolchain:** C++23, CMake, vcpkg. Parts are in Rust: LibWeb's, LibJS's (its JIT among them) and the compositor's.
  - **About 40 C/C++ libraries,** among them Skia, ICU, ffmpeg, OpenSSL, curl, sqlite, harfbuzz, freetype and fontconfig.
  - **The OS layer is POSIX:**
    - `Core::System` wraps 88 calls;
    - IPC goes over `socketpair` with descriptors passed;
    - the event loop is `poll`, a self-pipe and `pthread` keys;
    - `thread_local` is used 116 times.
  - **The Windows port's platform layer is 7,317 lines:** the size of a non-POSIX port's own part, over a full C runtime and threads.
  - **Painting:** Skia, on the GPU or on the CPU with `--force-cpu-painting`.
  - **The UI is Qt alone.**
- **Its official package,** 2026.48.0 for amd64 (`apt.ladybird.org`, SHA-256 71b6b62b…c45), unpacked and not run:
  - **329 MB in all.**
  - **The binaries:**
    - `WebContent` is 94 MB stripped: its `.text` is 75.7 MB, and it has a TLS segment of 15 KB;
    - `Hub` 26.8 MB, the Qt UI 24.6, `Compositor` 24.3, `ImageDecoder` 15.9, `RequestServer` 14.2;
    - each holds Ladybird's own libraries itself, and links glibc, libstdc++, ICU and Vulkan dynamically.
  - **ICU's data and code:** 34 MB.
- **MIND Core today:**
  - its largest program is `tls.elf`, 1.45 MB; its 112 programs together are 30 MB;
  - **the kernel refuses** an image over 8 MiB, a TLS segment and an interpreter (`kernel/src/elf.rs`);
  - **workers, not threads** ([262-KRN-0163](../issues-done/262-KRN-0163-workers-on-several-cpus.done));
  - only `init` and the loader start programs;
  - no libc, C++ runtime or POSIX layer;
  - no GPU API.

## The rule: no other language in the OS

- **The OS stays Rust:** the kernel, the services, `libmind`, the shell and `wm`.
  - They gain only what is language-neutral and needed anyway: images over 8 MiB (large inference modules need them as well), threads, and an application starting its own helpers.
- **Everything foreign is in `ports/ladybird/`:**
  - it holds a script, a pin of Ladybird's commit and its SHA-256, and patches; no source is copied in;
  - it is built by its own script and its own CI group, never by `02_build.sh`;
  - its output is a signed application package (750-UPD-0019) installed into `data/apps/ladybird/`.
- **The POSIX layer is Rust,** linked into ported programs alone: Redox's `relibc` is evaluated first, as 650 says. The C/C++ is Ladybird's and its libraries', nothing else.
- **No sandbox code is ported.**
  - Each of Ladybird's processes is a MIND task holding only its channels.
  - So MIND's capabilities stand where Linux's seccomp and Landlock stand.
  - Ladybird's memory safety is then the content task's own concern (MC-11.1, 11.8), and its profile says so.

## When it starts

It starts when every row below is done in `main` and the maintainer says so. Until then nothing of Ladybird enters the repository, not even `ports/ladybird/`.

| Track | What it gives | Recorded | State (2026-11-27) |
|---|---|---|---|
| `KRN` | images over 8 MiB (also for large inference modules) | `requests-KRN.md`, "Programs larger than 8 MiB" | asked |
| `KRN` | `PT_TLS`; threads in one address space and a wait on a memory word; an application starting the helpers its package names; a `vfs` file mapped with private copy-on-write pages; a larger address space and memory quota | the same request's second item, and 650 stage 3's list | to be asked when DEV's stage 3 starts |
| `DEV` | a C/C++ cross toolchain for `x86_64-unknown-mind` and `aarch64-unknown-mind`: clang, libc++, libunwind, compiler-rt, static | [650](650-building-on-the-target.md), stage 2 | not started (the track is open; it starts after the Ryzen's network) |
| `DEV` | the POSIX layer in Rust over `libmind`, and Rust's `std` for MIND on it | 650, stage 3, whose acceptance is a set of C and Rust `std` programs passing in QEMU | not started |
| `NET` | a grant broad enough for browsing, and names resolved, for the package's network process | [255](255-text-web-browser.md)'s and 256's ask | to be asked when 255 or this starts |
| `UPD` | a package of about 330 MB, with files up to about 100 MB and possibly more than `apppkg`'s 256 files | [750-UPD-0019](750-UPD-0019-signed-application-packages.md) | to be checked |

Nothing is asked of `APP`: `wm` already shows a window of a program's pixels at any size.

## Plan

**Steps 1 and 2 are the prerequisites above; steps 3 to 5 are `LBD`'s own,** numbered `264-LBD-0001` and on when the track starts.

1. **The kernel (`KRN`, Rust):**
   - images up to some hundreds of MiB;
   - TLS (`PT_TLS`), and threads in one address space with a futex;
   - an application's package naming the helper programs it may start.
2. **The toolchain and the runtime (`DEV`, in its own directories, never in the OS's):**
   - clang for `x86_64-unknown-mind` and `aarch64-unknown-mind`, static, with libc++, libunwind and compiler-rt;
   - the Rust POSIX layer over `libmind`: files, `mmap` over `MEM_SHARE`, threads, time, sockets over `netstack`, `poll`, spawn and wait;
   - Rust's `std` for MIND on it, for Ladybird's Rust parts.
3. **The libraries,** with a vcpkg triplet `x64-mind`:
   - without Qt, D-Bus, Vulkan, ANGLE or WebRTC;
   - Skia on the CPU alone.
4. **Ladybird's MIND platform,** as patches meant for upstream:
   - `Core::System`;
   - an event loop on MIND's IPC wait;
   - `IPC::Transport` over endpoints, passing capabilities;
   - spawning through step 1's grant;
   - a small native UI over LibWebView in a `wm` window, not Qt.
5. **Milestones,** each a QEMU suite on x86, then aarch64, then the Ryzen:
   1. `js` and `test262-runner`;
   2. `HeadlessWebView` paints a page into a PNG;
   3. the browser in `wm`;
   4. RequestServer over MIND's `tls`, which may wait.

## Port or our own browser

| | Ladybird ported | Our own (256) |
|---|---|---|
| **What we write** | Steps 1 to 4: the kernel's part (needed anyway), a Rust POSIX layer, a platform layer of the Windows port's size, a small UI | An HTML parser, CSS, layout, text, a JS engine and the DOM. Ladybird's LibWeb and LibJS are 1.36 million lines after six years of a funded team. |
| **What we get** | A browser for today's sites, at the pace of upstream (pre-alpha, its alpha planned for 2026) | A browser whose every line is Rust and whose standards are current alone, after years |
| **What it costs the OS** | Nothing foreign in the OS; 329 MB of package; about 100 MB a site's process maps; the CPU paints | Nothing foreign; the size it grows to |
| **Risks** | The toolchain and POSIX layer are the hard part. Upstream changes daily. C++ memory safety is held in by isolation alone. ffmpeg's LGPL. | It may never reach today's web. Static pages and the text browser come first. |

**The tools track's view:**
- **For a browser that opens today's sites, the port is far shorter:** we write tens of thousands of lines rather than more than a million.
- **Its step 1 is wanted anyway.** Large inference modules need the same images and threads, so it is the rehearsal the maintainer names.
- **Our own engine stays the long road:** 255's text browser and 256's static pages first. Nothing here makes 256 smaller.

## Acceptance criteria

- **Each milestone of step 5** as a QEMU suite, with its configuration named (MC-12.1).
- **The OS's own directories hold no C or C++:** a check of the tree outside `ports/`.
- **The package** is installed and removed as any application.
- **Ladybird and every library** are in THIRD_PARTY.md, with their licences.

## Related

[255](255-text-web-browser.md), [256](256-graphical-web-browser-current-standards-only.md), [650](650-building-on-the-target.md), [262-KRN-0163](../issues-done/262-KRN-0163-workers-on-several-cpus.done), [750-UPD-0019](750-UPD-0019-signed-application-packages.md).
