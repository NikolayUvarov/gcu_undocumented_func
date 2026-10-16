# RTL8852BE country catalog and 2G20 selection

Issue: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
This is a pure bounded `no_std` library. It performs no RF, firmware, PCI, IPC,
persistence, association or channel-switch operation.

## Source and complete inventory

The checked-in [wireless-regdb input](../rtw89_wifi/regdb/README.md) pins official
release `master-2026-09-03`, commit
`389b9b702cf9018e9a9078ccc4fa8eaae2e054e3`, with its exact ISC notice. The
generator validates all 1,016 source rules and eight ETSI WMM access categories.
The catalog contains all 182 source entries: 181 country/territory codes and
the separate world domain `00`. It includes historical `AN`; it is not a claim
to contain every currently assigned ISO country. Names/translations are not
supplied by this upstream database; the library enumerates the source codes.

All source bands, named flags, WMM references, DFS-region metadata and source
line numbers are retained. Equivalent rules share 244 generated records, while
all 1,016 per-country occurrences remain identifiable. In particular, Iran and
Oman's `NO-INDOOR` 5 GHz records remain present, even though upstream's binary
compiler omits those records. Retaining all bands for inspection does not add
5/6/60 GHz support to this driver.

Frequency/bandwidth uses exact integer kHz. EIRP uses signed mBm (hundredths of
dBm); decimal milliwatt values are converted with 80-digit decimal arithmetic
and rounded down. Thus 500 mW becomes 2,698 mBm, then at most 107 quarter-dBm or
53 half-dBm. Signed wide values preserve 36 dBm instead of overflowing an
eight-bit quarter-dBm field. EIRP is never silently relabeled as conducted
power, gain or SAR. `AUTO-BW` is retained but does not combine ranges or relax
the fixed 20 MHz fit check.

Flag meanings follow the primary
[nl80211 source](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/include/uapi/linux/nl80211.h#L4674)
and the upstream
[ASCII database format](https://wireless.docs.kernel.org/en/latest/en/developers/regulatory/wireless-regdb.html#ascii-file-format).
Internal named flags deliberately do not expose the Linux bit ABI or the
different compact `regulatory.db` flag encoding. No GPL implementation is
copied into the library.

## Selection and driver intersection

`regulatory::countries()` returns the complete sorted source catalog.
`select([u8; 2])` accepts an exact uppercase catalog code, rejects unknown or
malformed input, and rejects `00` as an operator country. `world()` exposes
that special profile separately for inspection. There is no inferred/default
country, case correction or unknown-country fallback. A `Selection` preserves
the code and exact source commit/database hash; policy epochs are still the
future consumer's responsibility.

`Country::channels_2g20()` enumerates covered 20 MHz channels, including passive
or conditional ones, with EIRP/flags/source-rule provenance. Coverage requires
the whole center +/-10 MHz interval to fit a rule whose maximum bandwidth is
at least 20 MHz. Channels 1–13 use 2412–2472 MHz; channel 14 uses 2484 MHz.
Future overlapping full-width matches are rejected during generation rather
than selecting the most permissive rule.

`Channel::assess_initial_driver(Environment)` exposes the current limited
consumer intersection: channels above 13, DFS, prohibited OFDM/CCK, unsupported
link-topology restrictions and unimplemented WMM requirements are explicit
unsupported results. Indoor/outdoor restrictions require a compatible resolved
environment; unknown environment cannot satisfy them. `NO-IR` remains
`PassiveOnly`. Source `NO-HT40` does not restrict this 20 MHz path. A country's
DFS-region field does not impose DFS on its non-DFS frequency rules.

`EligibleForIntersection` is frequency/mode data, **not a TX capability**.
This module cannot construct the existing `phy::channel::Rule`, resolved
platform policy, calibration authorities or an active channel owner. Actual
board country and programmed-country equality, source chip limits, verified
firmware/default-table provenance, supported band/rate, antenna gain and
platform SAR must still be resolved/intersected by the checked provider. A
regulatory entry can exist while the current driver/platform cannot use it.
Selection does not bypass these constraints, and read-only raw source rules
are not alternative runtime activation inputs.

## Explicit deployment choice

On 2026-10-16 the operator stated the physical country is **UZ (Uzbekistan)**
and requested country/channel switching using the complete checked database.
This records UZ as the requested deployment setting for that environment;
it is not a compiled default for every MIND installation. No channel was
selected implicitly. UI, persistent versioned configuration, operator change
handling and native consumption are separately assigned integration work.

Pinned UZ rule: 2402–2482 MHz, maximum bandwidth 40 MHz, EIRP 20 dBm, no 2 GHz
rule flags. Its 20 MHz coverage is channels 1–13, including both exact range
edges; channel 14 is outside. These are database-derived limits, not measured
radio output or a hardware-success claim. The older Ubuntu observation of DE
does not override the operator's explicit UZ choice or establish board OTP.

## Validation state

Before publication: the native crate and host harness are compiled, with
strict parser/source-reproduction and consumer-boundary fixtures registered
in `scripts/host_tests.sh`. Executions wait for the project's raw-publication
step; results will be appended with exact commits. No hardware or new full
gate evidence is claimed by this slice.
