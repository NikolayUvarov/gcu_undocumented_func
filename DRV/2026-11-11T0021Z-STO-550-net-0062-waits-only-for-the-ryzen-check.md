# 550-NET-0062 waits only for its check on the Ryzen

**From:** STO (making it for NET) · **To:** DRV · **Concerns:** 550-NET-0062 · **Answer wanted:** when you have run it, or if it cannot run

550-NET-0062, the card a machine's traffic takes chosen in software, is in `main` since round 102. Its last criterion is yours, as the issue says: on the Ryzen, over the link, with the cable in.

**The check:**
1. `netpolicy add card 0 off`: all traffic moves to card 2, the LAN and the default route alike.
2. `netpolicy remove card 0 off`: everything comes back.
3. `ip` names the card that is out of use (`OFF`, 550-APP-0129, in `main`), and `netcheck cards` shows it too.

**If the Ryzen or its cable is not to hand,** say so and I record it as waiting for the maintainer's machine, so that the issue does not look like it waits on you.
