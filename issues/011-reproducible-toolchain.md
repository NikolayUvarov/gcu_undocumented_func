# 011 — Воспроизводимая сборка: `rust-toolchain.toml`, `Cargo.lock`, workspace

**Тип:** infra · **Приоритет:** низкий · **Статус:** open
**Затрагивает:** корень репозитория, `01_prepare_env.sh`, `.gitignore`

## Проблема

- `01_prepare_env.sh` делает `rustup default nightly` — меняет глобальный тулчейн пользователя, и nightly при этом «плавающий»: сегодня `1.100.0-nightly (2026-09-14)`, завтра другой. Флаг `-Z relax-elf-relocations` (issue [001](001-flat-binary-entry-offset-and-got-call.md)) и `abi_x86_interrupt` — nightly-фичи, поведение может измениться.
- `Cargo.lock` в `.gitignore` — для бинарных крейтов его принято коммитить; `uefi` зафиксирован `=0.27.0`, но `bitflags`, `log`, `syn` и т.д. подтягиваются свежими.
- Три отдельных крейта без workspace: три `target/`, три lock-файла, `BootInfo` продублирован.

## План

1. Корневой `rust-toolchain.toml`:
   ```toml
   [toolchain]
   channel = "nightly-2026-09-14"
   components = ["llvm-tools-preview", "rust-src"]
   targets = ["x86_64-unknown-none", "x86_64-unknown-uefi"]
   ```
   и убрать `rustup default nightly` из `01_prepare_env.sh` (rustup сам подхватит файл).
2. Убрать `Cargo.lock` из `.gitignore`, закоммитить lock-файлы.
3. Workspace: корневой `Cargo.toml` с `members = ["bootloader", "kernel", "app", "mind-abi"]`; разные таргеты у членов — через `.cargo/config.toml` в каждом крейте (как сейчас) либо `-Z per-package-target`. Если workspace мешает (разные таргеты в одном `cargo build`), оставить отдельные крейты, но общий `mind-abi` подключить как `path`-зависимость.
4. CI (GitHub Actions): `02_build.sh` + проверки из issue 001 (`Entry 0x0`, нет релокаций).

## Критерии готовности

- Чистый клон + `rustup` без ручных действий собирается одной командой.
- `git status` чистый после сборки (артефакты только в игнорируемых путях).
