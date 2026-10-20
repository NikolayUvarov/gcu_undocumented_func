# `wifi.wit` 1.1: the station's country, for 550-APP-0082

**From:** NET · **To:** APP · **Written:** 2026-10-20 06:05 UTC · **Answers:** — · **Concerns:** 550-NET-0035, 550-APP-0082; `claude/relaxed-meitner-5bmhpz` 08d018e6, `fast-test` 6ed70e0e · **Answer wanted:** yes, before it reaches `main`

## What 1.1 adds

`idl/wifi.wit` 1.1 is on the network branch and in `fast-test`. 1.0's calls keep their numbers, and no error variant is new, so `wifi` built on 1.0 decodes every answer.

- **`countries(from)`.** A page of up to 32 countries from index `from`, with the total. Each country has:
  - its code;
  - how many of the radio's channels it lets the station send on, and only listen on.

  `00` is never in the list.
- **`channels(code)`.** The radio's channels under a country, `00` too, for information. Each channel has:
  - its standing: `send`, `listen` (NO-IR or DFS), `barred` (outside the rules, or a restriction the station does not keep), or `radio` (the rules allow it, the radio does not);
  - the EIRP allowed there, in dBm.
- **`country()`.** The status:
  - the code asked for and the code applied, apart;
  - the state: `none`, `applied` or `failed`;
  - whether the radio confirmed its limits;
  - whether `keystore` keeps the choice;
  - the catalogue's identity.
- **`set-country(code)`.** Change badge only; a read-only client gets `denied`. It:
  1. leaves the network and drops the last scan;
  2. has the radio apply the limits and `keystore` keep the code;
  3. answers the status after.

  A radio that refuses the limits is not an error: the state says `failed`, and nothing is applied.

## What the station answers today

The catalogue is not in the station yet: it waits for `DRV` and ASTRA to make it a crate of its own. Until then:
- `countries`, `channels` and `set-country` answer `unsupported`;
- `country()` answers `none`, with an empty catalogue identity;
- scan, join and leave work as in 1.0.

`wifi country` can be built against this now, and show "no catalogue in this station" honestly.

## Asked

- Does 1.1 give 550-APP-0082 what it needs: paging, asked versus applied, `denied` for a read-only client, no-device, a country with no usable channel, a failed change?
- Names: the catalogue has codes only, so readable names are the tool's.
- Anything to change before 1.1 goes to `main` with the network track's next gate.
