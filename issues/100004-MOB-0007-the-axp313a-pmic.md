# 100004-MOB-0007 — The AXP313a PMIC on the Orange Pi Zero 3, with an interface ready for the AXP803

**Type:** driver (the phone's own chip) · **Owner:** `MOB` · **Priority:** P3 · **Status:** open · **Blocked by:** [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md), [100004-MOB-0006](100004-MOB-0006-i2c-and-the-h5s-sensors.md) (the TWI driver, for R_I2C) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md); [100000](100000-a-phone-the-pinephone-first.md), stage 2 (the AXP803); [100002](100002-power-on-a-battery-idle-frequency-and-sleep.md) · **Constitution:** MC-2.3, MC-6.3, MC-12.1, MC-12.3

## Problem

**The PinePhone's AXP803**, on RSB, gives:
- the power key, the battery and its charging;
- the regulators for the display, the modem and the cameras.

**The Orange Pi Zero 3's AXP313a** is a much smaller chip, on I2C (R_I2C, 0x07081400): three DCDC outputs and an LDO. It is enough to shape the interface and the safety rules before the phone.

**A PMIC can damage the board.** A wrong voltage on the DRAM's or the CPU's rail does it, so the driver changes only what its policy allows.

## Plan

1. **The interface,** `idl/pmic.wit` 1.0:
   - **calls:** the rails and their state; a rail set within the limits the board's tree gives (`regulator-min-microvolt`, `regulator-max-microvolt`); the power key's events and the battery later;
   - **one interface, several chips:** the AXP313a first, the AXP803 with the phone.
2. **The AXP313a's driver:**
   - it reads the chip's ID and the rails' state;
   - the CPU's and the DRAM's rails are read-only, unless the maintainer allows otherwise;
   - writes are refused outside the tree's limits.
3. **Host tests** against a register model: limits, refusals, the ID check.

## Acceptance criteria

- **On the H618:** the rails read and match what U-Boot set (its log, or a voltmeter).
- **A rail outside its limits** is refused, and nothing is written.
- **What is not tested** is listed. The AXP803 is not covered until the phone (MC-12.1).

## Related

[100004-MOB-0006](100004-MOB-0006-i2c-and-the-h5s-sensors.md), [100002](100002-power-on-a-battery-idle-frequency-and-sleep.md).
