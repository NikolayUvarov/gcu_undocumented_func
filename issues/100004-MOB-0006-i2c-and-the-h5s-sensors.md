# 100004-MOB-0006 — The I2C controller (TWI) and sensors on the H5's header

**Type:** driver (the phone's own chip) · **Owner:** `MOB` · **Priority:** P3 · **Status:** open · **Blocked by:** [100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md); the sensors at hand (the handoff names an MPU-6050 and a QMC5883L; which are wired is the maintainer's) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md); [100000](100000-a-phone-the-pinephone-first.md), stage 2 (the touchscreen and the sensors are on I2C) · **Constitution:** MC-2.3, MC-12.1

## Problem

**What hangs on I2C on the PinePhone:**
- the touchscreen (Goodix GT917S);
- the accelerometer and the magnetometer;
- the light and proximity sensor;
- the cameras' control.

**The controller** is Allwinner's TWI (Linux's `mv64xxx-i2c`, `allwinner,sun6i-a31-i2c`). On the H5 it is at the A64's addresses (TWI0 0x01C2AC00).

## Plan

1. **A service, `sunxi_i2c`,** with a new interface `idl/i2c.wit` 1.0. Its parts:
   - **the calls:** transfers to an address, with a write, a read, or a write then a read;
   - **the node:** bound by its `compatible`, the pins as U-Boot or the tree's pin settings leave them;
   - **the badge:** each client may reach only the addresses `init` gives it (MC-2.3);
   - **each transfer:** a deadline, and an arbitration loss or a NACK answered as such.
2. **A first client:** a tool that reads a sensor's identity register (the MPU-6050's `WHO_AM_I`, the QMC5883L's chip ID) and a few samples.
3. **Host tests** of the controller's state machine against a model.

## Acceptance criteria

- **On the H5:** the sensor at hand answers its identity and gives samples that change when the board moves.
- **A client** that asks an address outside its badge is refused (`rights`).

## Related

[100004-MOB-0007](100004-MOB-0007-the-axp313a-pmic.md) (the same controller family as R_I2C on the H618), [100000](100000-a-phone-the-pinephone-first.md).
