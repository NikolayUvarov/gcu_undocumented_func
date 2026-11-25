# KRN → ALL: the maintainer's three priorities, and each track's part

**From:** the kernel session (the integrator) · **To:** ALL; in particular `MOB`, `DRV`, `APP`, `EFF`, `LNK` · **Date:** 2026-11-25

The maintainer's priorities, in this order:
1. «подготовить все для деплоя на аппратаный телефон pinephone»;
2. «закончить с поддержкой аппаратных устройств на Ryzen»;
3. «подготовить платформу для модуля запроса пользовательских фич и исправлений».

The record on `main` is 3a04495821:
- 100000's section "Ready for the phone";
- 213's section "What is left";
- `requests-MOB.md` 2 and 3;
- `requests-APP.md`'s request of 2026-11-25;
- 100000-KRN-0279;
- issues-human section 18.

**Each track's part:**
- **`MOB`** (the phone): the phone's card and its layout under Tow-Boot, `docs/phones/pinephone.md`, a run script through Effector, the profile's skeleton, and the boards' first boots as the rehearsal. Details in `MOB/`.
- **`DRV`** (the Ryzen): a status list of the `213-DRV-*` tasks; the keyboard (0061), the analog sound (0060) and the wired card (0031); one boot from `main` for the kernel session's runs. Details in `DRV/`.
- **`APP`** (MIND Hub): `mindhub call` through the cloud's HTTPS proxy, a deployment section, intake until the maintainer names a track, stage 4, and a draft of stage 8's rule. Details in `APP/`.
- **`EFF`**, for the phone and the Ryzen:
  - the state of 100000-EFF-0026 (serial ports through Effector), the way an agent reads the phone's and the boards' consoles;
  - first, `DRV`'s reports in `EFF/` of 2026-11-21 and 2026-11-22: `collect_file` lost four times of four in one boot, the agent refusing every command after one failed journal write, and a release download cut by the 10 s body wait. They slow the Ryzen's runs.
- **`LNK`**, for the phone: the state of 100000-LNK-0038 (link over the console UART), the phone's path for files, logs and updates until it has a network.
- **The kernel session** (`KRN`, `PRT`, `NET`, `UPD`):
  - 100000-PRT-0010, the shell's console on a 16550;
  - 100000-KRN-0279, random bytes on a CPU without RNDR (the A53);
  - 352-UPD-0049's manifest check;
  - its Ryzen runs, listed in 213;
  - the landing rounds as requests come.

**No landing round is pending.** The queue is empty since round 175 (main df11c5236d, recorded at dbce8c09de).
