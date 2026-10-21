# A request: the developer mode, the word `dev` in `link.conf`, and the grants it needs from `init`

**From:** LNK · **To:** KRN (NET for item 1) · **Written:** 2026-10-21 · **Answers:** — · **Concerns:** `init/src/main.rs`, the `link` arm; 800-LNK-0012 · **Answer wanted:** yes, when you number it

## The maintainer's decision

On 2026-10-21 the maintainer asked: "Can we add debug developer mode to allow full normal debug using this effector link? Add necessary words to link.conf to be able do the tests."

The reason: the agent on the other end of the cable should test the MacBook Pro's Wi-Fi by itself. That means joining the network, pinging and restarting, with no one at the keyboard. Today a program that `link` starts gets an application's standard grants and nothing more. It cannot reach the network or the station.

## The word

`dev` in `link.conf`. The maintainer's `build-and-write.sh --link-dev` writes `baud=460800 run dev`.
- **Without `dev`, nothing changes:** `link` keeps your four grants and its one endpoint.
- **With it,** `link` may hand its programs what the shell would hand them, without the shell's question on the screen:
  - whoever holds the cable can then debug the machine as its user would;
  - it is for the maintainer's own machines;
  - `docs/profile/bootstrap.md` should say so in `link`'s row.

## The grants wanted with `dev`

Proposed slots below; the choice is yours.

1. **A network policy broker client like the shell's,** in `SLOT_NETPOLICY` (19).
   - With it, `link` asks a flow grant for each program that requests `REQUEST_NETWORK` and lends it in `SLOT_NETWORK`, as the shell does (`netpolicy::prepare`, `take`, `bind`, shell/src/net.rs).
   - NET may want a rule for it, or a badge, so the policy knows those flows come from the link. I will tell NET.
2. **The Wi-Fi station's configuration client with `BADGE_WIFI_CHANGE`,** in `SLOT_WIFI` (33). `link` lends a copy to programs that request `REQUEST_WIFI` (`wifi join`, `wifi status`).
3. **A lifecycle client badged `BADGE_REBOOT`,** in `SLOT_LIFECYCLE` (11), as the updater's (init/src/main.rs, its arm). It serves a REBOOT request of the link protocol.
4. **The system monitor's client,** in `SLOT_SYSINFO` (10), lent to programs that request `REQUEST_SYSINFO` (`uptime`, `top`, `netcheck cards`).

## What the link track does meanwhile (800-LNK-0012)

- **`link`** reads `dev` and lends 1, 2 and 4 to its programs when it holds them, by `cap_info`. Without them it does as now.
- **With `dev`, a RUN of a plain name** without `/` or `.elf` (`wifi`, `netcheck`) starts the built-in program of that name from the volume's root through your loader. Without `dev`, every name stays a path under `data/link/`.
- **A REBOOT request,** a new kind on the control channel, uses 3. Without it, REBOOT is refused with UNSUPPORTED.
- **The `link` suite** checks each with the grants, once they are in `fast-test`, and checks the refusals without `dev`.

A launch session takes at most 5 grants. These need at most 4 (console, network, Wi-Fi, system monitor).
