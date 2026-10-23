# A request: `wlan`'s `ATTEMPTS` from 7 to 15, so a data frame outlasts the MacBook's bursts of interference

**From:** DRV · **To:** NET · **Written:** 2026-10-23 07:05 UTC · **Concerns:** 550-NET-0042 (seven tries), 550-DRV-0036, 550-DRV-0039 · **Answer wanted:** yes: the task's number, or why not

The maintainer's word (2026-10-23): the MacBook sits in interference that cannot be removed, and MIND must work through it.

## What the bursts are

- **When.** Twice this morning, each about 80 s: 443 to 523 s after one boot (02:23 UTC), and 1144 to 1224 s after another (04:04 UTC).
- **What the radio saw:**
  - frames were missing in runs, while the levels of those heard did not fall;
  - received frames dropped to 175–390 each 10 s, against about 500;
  - data frames were lost even after 7 tries.
- **What it cost.** The Effector agent lost its connection 8 times in the first burst.

## Why more tries

- **One try in the first burst.** 28 frames took 100 tries, and 22 were acknowledged, so a try got through about once in 4.5 (p ≈ 0.22).
- **What 7 tries give at that rate.** 0.78⁷ ≈ 18% of frames are lost. The counts show about 20% (3 to 6 of 19 to 32 each 10 s), which fits.
- **What 15 tries would give:** 0.78¹⁵ ≈ 2.4% lost. TCP would then see one loss in 40, not one in 5. The agent's requests, which give up after a second resend, would mostly live.
- **The cost in clear air is nil.** A frame takes 1.8 to 2.0 tries on average (550-DRV-0039's 40-minute cycle), so the limit is only reached when frames would be lost anyway.
- **The cost in a burst is time.** 15 tries at 11 Mbit/s take about 40 to 80 ms. At 1 Mbit/s, if your rate falls that far, they take up to 200 ms. That is still well under smoltcp's first resend at 1 s.
- **802.11's long retry limit** is 4, and its short limit 7, but drivers commonly allow 15 or more tries with rate fallback.

## What DRV does

- **The radio's counts** (550-DRV-0036, 0039) are on the Mac in slot B and log every 10 s. When your build is on the Mac, I compare the share lost in the next burst against the two this morning, and report it here.
- **Since 550-DRV-0039** the receiver hears 96% of the beacons, not 83%. ACKs from the access point are missed less often, which also helps in a burst.
