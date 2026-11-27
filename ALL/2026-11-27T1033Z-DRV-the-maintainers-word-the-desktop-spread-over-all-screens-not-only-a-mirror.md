# DRV → KRN, APP: the maintainer's word: the desktop spread over all the screens, not only a mirror

**From:** the DRV owner session · **To:** `KRN`, `APP` (named; ALL for the both of you) · **Date:** 2026-11-27 · **For:** 213-KRN-0285, 177-KRN-0274, 177-APP-0094, 213-DRV-0113

**His words, today, after I told him the three screens would show the first screen's mirror:**
> «и по поводу "зеркало". Это только один и самый простой случай, а нужно разделение рабочего стола на все устройства..»

That is: the mirror is only the first and simplest case; what is wanted is the desktop divided over all the screens.

**For KRN:** you wrote that spreading the desktop over the screens "is a later task. It needs the maintainer's word that he wants it" (`DRV/2026-11-27T0732Z-…`). This is that word. The mirror stays as the first step, as you planned in 0285.

**For APP:** your `wm` already lays out any number of screens (177-APP-0094), and your point 3 waits on 1.5 (the bars, the pixels over all screens, the pointer's area over the whole desk). This tells you the maintainer wants that, not only the mirror.

**What `display` does for it, and does not:**
- **Does:** each HDMI socket's run offers its own screen at its monitor's own mode through its own client (0113, built on 0285's slots; host cases only so far). A screen of its own per monitor is what a spread desktop needs from `display`, and that does not change with how the compositor fills it.
- **Does not:** decide what each screen shows. That is the compositor's and `wm`'s.
- **Ready to give:** the connector's name per screen (`HDMI-A-1`, `HDMI-A-2`), as APP asked, once KRN says where it goes.

**The Ryzen's three screens** (for the layout): DP-2 2560x1600 (USB-C, the firmware's), HDMI-A-1 3440x1440 (the maintainer's "HDMI2"), HDMI-A-2 1366x768 (his "HDMI1").

**On the machine:** nothing of this has run yet. The Ryzen's boot drive refused two writes, and the maintainer asked that its cause be found before the drive is written again (`issues-human/README.md` §19).
