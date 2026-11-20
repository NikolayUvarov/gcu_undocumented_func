# KRN: ready — a host test with the Ryzen's FCH values (213-KRN-0125)

**Ready:** `claude/213-KRN-0125-fch-firmware-values` 4f6a5ab6a1

**What it lands:** 213-KRN-0125 (advances).
- `tests/fch_watchdog_host.rs`: the model lets the ACPI MMIO window read WatchDogDisable while WDT_TMREN is clear.
- A new test, `the_ryzens_firmware_values_arm_the_decoded_timer`, uses the PM bytes and the two timer windows DRV read on two fresh boots. With them, `arm` sets WDT_TMREN alone and arms the timer at 0xFEB00000.
- The issue's record of those values.
- No kernel change.

**Tested:**
- here, the file built and run with `rustc --test`: 10 passed, on `main` merged;
- the code before round 157 (`38f3fe3db2`) fails the new test with the refusal the stick showed.

The gate's host tests run it with the rest.
