# 100001 — Telephony: the modem as another computer; SMS and calls from the console

**Type:** main task (a service and tools) · **Owner:** `MOB` track (later) · **Priority:** P2 · **Status:** an intention · **Blocked by:** [100000](100000-a-phone-the-pinephone-first.md) stage 2 (USB, and the PMIC's and the modem's power lines) · **Roadmap:** track H; track D (an external party through an adapter) · **Constitution:** MC-3.1, MC-10.3, MC-11.3, MC-11.4, MC-11.5, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the kernel session, 2026-10-25, with main task 100000, at the maintainer's request.

## Problem

- **The PinePhone's modem** (Quectel EG25-G) is a separate computer with firmware that is not ours.
  - It is connected by USB, and powered and reset through the phone's pins.
  - It is driven by AT commands over its USB serial ports.
  - Voice goes over a PCM link to the A64's codec.
  - Data goes over QMI, MBIM or PPP.
- **Nothing in MIND Core sends a text message or makes a call.**

## Plan

- **A service `modem`** (a new directory) with `idl/modem.wit` 1.0:
  - power on and off, through `gpio` (207) and the PMIC;
  - the SIM's state, registration and the signal;
  - SMS sent and received, in PDU mode;
  - calls: dial, answer and hang up, with the sound routed through the codec;
  - data later.
- **Parsing:** what the modem sends (AT responses, PDUs) is parsed in a parser session that holds nothing else (MC-11.11). A message's text is data, never an instruction (MC-11.5).
- **The USB side:** the modem's serial ports, as a class driver of `usb_host` (a `DRV` task).
- **Authority:**
  - only `modem` holds the modem;
  - programs get badges to send SMS, read SMS and call;
  - every SMS sent and every call made is logged (MC-10.3);
  - the SIM's PIN is kept in `keystore` (000-NET-0045).
- **Console tools:**
  - `sms send <number> <text>`, `sms read [--follow]`;
  - `call <number>`, `call answer`, `call end`;
  - a full-screen view later, on `mind::tui`.
- **The modem's firmware is not ours:** it is not updated from MIND Core, and its version is reported.
- **Personal data:** messages and numbers are stored only once the store is encrypted at rest (308, proposed to `STO`). Until then they are held in memory.
- **Not provided, said wherever the tools describe themselves:** emergency calls. This phone is not to be relied on for them (MC-12.3).

## Acceptance criteria

- **On the host:** tests of the AT and PDU parsers, and a fuzz target for each (`ASR`'s harness).
- **On the PinePhone,** with its configuration named (MC-12.1):
  - the modem powered and registered;
  - an SMS sent and one received;
  - a call made and one answered, with sound both ways;
  - the modem's kill switch turned off: the service reports the modem gone.

## Related

[100000](100000-a-phone-the-pinephone-first.md), [207](207-gpio-service.md), [000-NET-0045](000-NET-0045-secrets-per-program-in-keystore.md), [950](950-our-own-micro-messenger.md) (the console tools' manner).
