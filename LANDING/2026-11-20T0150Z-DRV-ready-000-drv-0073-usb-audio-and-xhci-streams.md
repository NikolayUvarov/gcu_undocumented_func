# DRV landing request: isochronous streams on xHCI, USB Audio Class 1 in audio_gw, a failed controller set up again (000-DRV-0073, with 213-KRN-0263)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-20 01:50 UTC · **Concerns:** 000-DRV-0073, 213-KRN-0263, 158-DRV-0072, 801 S5

**Ready:** `claude/000-DRV-0073-usb-audio` 1d94dc783

## What it carries

On `main` bf0279223 (merged in). It holds 158-DRV-0072's commits as well (its own request: `2026-11-19T2328Z-DRV-ready-158-drv-0072-every-usb-camera.md`). It is in `fast-test` da321698b.
- **`usb_host`:**
  - isochronous streams on xHCI, IN and OUT, each TD on its frame as Linux queues them;
  - a Configure Endpoint that copies the device's slot and swaps an interface's endpoints;
  - **a controller that stops with an error is set up again by itself**: its devices are found anew, and it takes no stream afterwards;
  - AMD controllers take no stream yet (below);
  - diagnostics: completion codes, stalls, and the raw context words once a stream.
- **`idl/usb.wit` 1.5:** `isochronous-out` and `mind::usb::BADGE_AUDIO`. `libmind/src/idl/usb.rs` was regenerated with `scripts/mind_idl.py`.
- **`init` (213-KRN-0263, numbered by you and carried here at your word):** `SLOT_AUDIO_USB = 8`, and `audio_gw` gets `usb_host`'s client badged for audio when `usb_host` runs.
- **`libmind` `uac`, `audio_gw`:** a UAC1 device in the codec's place while it is plugged in; the codec back when it goes or its stream is refused.
- **`tests/uac_host.rs`, and `--suites usbaudio`** in `x86: services, storage, audio` (`ci_local.sh` and `ci.yml`).

## Tested

- **On LEG9:**
  - `build (x86)`, `host tests`, `build (x86 test programs)`, `x86: keys, shell, tools` (usb, ehci), `x86: the updater and an update through the link` and `x86: USB image` pass;
  - the suites `services`, `audio`, `tts`, `listen`, `hda`, `usbaudio`, `link` and `modes` pass one by one.
- **Two things that fail here fail on `main` too:**
  - `ahci` fails here on `main` bf0279223 as well: `run files &` meets `CANNOT READ THE PROGRAM` after the driver's restart;
  - `sound_kept_check` failed once, then passed. The machines' gate decides those.
- **aarch64:** left to the gate.
- **On the Ryzen** (test releases through `data/link/rel`, each confirmed by its trial):
  - the STM-Speaker is attached and the cameras are listed;
  - its AMD xHCI (1022:1639) **stops with a host controller error on the first isochronous TD**, though every decoded context and TRB word matches Linux's, and Linux plays there;
  - the recovery brought the controller and the link's converter back within a second, every time;
  - hence the AMD gate: there the device falls to the codec at once.

000-DRV-0073 stays open for the AMD halt.
