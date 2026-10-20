# RTL8852BE pinned firmware table provenance

Task: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
This records the absence of appended firmware override elements in one
hash-pinned file. It does not authorize a channel, establish platform or
SAR/EIRP limits, attest current hardware readiness, or permit transmission.

## Source and observed metadata

Linux v6.19 `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`,
`fw.c::rtw89_mfw_get_size` and `rtw89_fw_recognize_elements`, obtains the
end of the final MFW directory entry, then aligns it to 16 bytes. It does
not take the greatest end among arbitrarily reordered entries.

The separately fetched private `rtw8852b_fw-1.bin` has SHA-256
`5b68415e3bfe72715d63a70703d4471b04b5475b8ff69cfdc8cdf48233cd5d3a`
and 1,245,944 bytes. Its complete directory has five images. The final
entry (cut 0, type 255) starts at 1,192,968 and spans 52,976 bytes, so both
the final and greatest image ends equal EOF. The aligned element offset is
1,245,952, beyond EOF. Thus this exact file has no appended firmware power,
country or dynamic antenna-gain elements. No private payload is committed.

## API boundary

`firmware::container_layout` validates every range, overlap and duplicate
selection key, preserving the final and maximum ends separately. This
structural parser accepts arbitrary bytes and makes no provenance claim.
The public downloader still requires the existing exact length, SHA-256,
measured board/security policy and selected-image metadata. Its private
parser additionally checks five entries and final=max=EOF before retaining
the layout in `Verified`. Successful actual download transfers it to `Ready`.

`Ready::default_tables()` produces an opaque `DefaultTables` value before
Ready is moved into the exclusive startup session. Its private session ID
and layout cannot be supplied by a production caller. Consumers must match
the actual command session and PHY attempt and perform their normal live
guards. `belongs_to` checks identity only: provenance remains historical
after shutdown, while all hardware-readiness checks must still fail.
Other pins have no public provenance constructor.

## Validation status

The existing registered firmware/downloader harnesses cover reordered last
entries, trailing data, incomplete directory coverage, invalid late entries,
and opaque-token rejection after reconstructing a queue at the same DMA
address. The synthetic fixture describes all five metadata ranges, while
its payload stays synthetic and cannot pass the public SHA-256 policy.
The explicit private fixture checks the real hash and obtains provenance
through the actual downloader model before checking a one-byte mutation.

Own-branch source commit `3ffb323c` built in release mode without warnings
in 0.97 seconds. After raw publication `1a62e46f`, the unchanged own branch
ran these focused checks on 2026-10-16 MSK (x86-64 WSL2):

- Firmware: all 14 synthetic cases passed in 0.01 seconds.
- Downloader: all 20 synthetic cases passed in 0.19 seconds.
- One-device cumulative MAC/RX: all six cases passed in 0.10 seconds.
- Both opt-in private-file tests, excluded from the ordinary synthetic
  runs, were explicitly invoked with the separately fetched file and
  passed: parser in 0.00 seconds, real-hash downloader/provenance/mutation
  case in 0.05 seconds. Neither private test remained unexecuted.
- The explicit private-file pipeline binary passed GEN1 and GEN2 download
  (164 packets each) and missing-H2C GEN2 (zero packets). Model times were
  12,344 / 12,344 / 407,344 microseconds; these are not hardware timings.

No selected case failed. The cumulative filter excludes unrelated imported
module tests. Independent read-only review found no blocking defect. These
fixtures and the private-file checks are host evidence only; no build or
test ran on `fast-test`. Physical board validation and the full gate remain
open.
