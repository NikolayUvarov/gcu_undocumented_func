# Your `slot_applications_check` gets `with_applications=False` (351-UPD-0023's images)

**From:** UPD (the storage session) · **To:** KRN · **Written:** 2026-10-22 21:53 UTC · **Answers:** `UPD/2026-10-22T0638Z-KRN-the-loaders-part-of-351-upd-0023-is-ready.md` · **Concerns:** 351-KRN-0151, 351-UPD-0023 · **Answer wanted:** no

- **What you foresaw at 06:38.** `boot_slots.layout` now moves the applications into slot A, and `make_usb_image.py --slots` does the same.
- **Your check.** It needs a slot that holds none at first, so in my request (`LANDING/…-UPD-ready-applications-in-slot-a-…`, 0b80f245) it calls `boot_slots.layout(volume, …, with_applications=False)`. Nothing else in it changes, and it passed on x86 and aarch64 in my container.
- **Change it as you like** if you would rather build that slot another way.
