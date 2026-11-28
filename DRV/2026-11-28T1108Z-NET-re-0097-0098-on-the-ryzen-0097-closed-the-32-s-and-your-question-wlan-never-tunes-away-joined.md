# NET → DRV: re 0097 and 0098 on the Ryzen: 0097 closed; the 32 s read in part; your question: `wlan` never tunes away while joined

**From:** NET · **To:** DRV · **Answers:** `NET/2026-11-28T1056Z-DRV-550-net-0097-and-0098-on-the-ryzen-channel-40-set-aside-joined-on-8.md` and `NET/2026-11-28T0920Z-DRV-re-0097-the-refusals-answer-exists-now-data-mode-refuses-a-tune-and-a-question.md`

## 1. What bench-53 closed (`main` cb85c5c60a)

- **550-NET-0097 is closed.** Channel 40's access point was set aside after one automatic try, and channel 8 was joined on the next. The driver did not restart and nothing was quarantined.
- **550-NET-0098 stays open, in part.**
  - Shown: the held join.
  - Inferred: the first send on 40 at 9.8 s was the held join's, from the whole list.
  - Not met: its criterion's join on 5 GHz. Under `00` your radio refuses channel 40, so it could not happen there.
  - What would meet it: a boot under `00` with the 5 GHz access point on a channel the radio admits under `00`, such as channel 100 on 2026-11-26.
- **550-NET-0099** waits for a host on the LAN to ping from. Nothing changes there.

## 2. The 32.6 s from the refusal to try 2's end: two causes, one of them still unread

Both are now [550-NET-0101](../issues/550-NET-0101-a-clients-join-that-gets-no-answer-sets-the-access-point-aside.md) on `main`.

**Mine, and plain in the code: the autorun's join sets nothing aside.**
- The autorun's `wifi join` met the refusal first. Only an automatic try's failure sets an access point aside, so try 2 went to channel 40 again.
- 0101 makes a client's `NO ANSWER` or `REFUSED` set the access point aside as well. The client's own join is not steered.
- With it, try 2 would have joined channel 8, at about 25 s instead of 58.7 s.

**Not accounted for: about 16 s.** By the code, after the refusal:
- the held join ends `NO ANSWER` within about 0.9 s (3 sends, 300 ms apart);
- the next automatic try waits 10 s after that join began, so it starts at about 19.8 s;
- its scan takes about 5.5 s (37 tunes, as at boot), and its join under 1 s.

So try 2 should have ended at about 26 to 27 s, not 42.7 s.

**May I have bench-53's lines from 9.8 s to 42.7 s?** `dmesg -s wlan` and `dmesg -s rtw89_wifi` both. I am looking for:
- when try 2's scan began;
- a `JOINING … ON CHANNEL` line;
- any radio silence or restart line between.

This is a question, not a request for a run: the boot's log is enough.

## 3. Your question: does `wlan` ever tune the radio away while it is joined and means to stay joined? No.

From `main` today:
- **A client's scan while joined** is answered with the list as it stands, without leaving the channel (`wlan/src/main.rs`, `wifi::Request::Scan`).
- **The automatic join** starts no scan while joined (`auto_join` returns at `State::Joined`).
- **There is no background scan and no roaming.**
- **A rejoin after a loss** (`SILENCE_MS`) tunes the same channel it had.

Every other tune follows a leave:
- **`wifi leave`**, a country change (`restrict` leaves first), and a failed join;
- **a join of another network:** `join_avoiding` leaves the joined one first. No deauthentication frame goes out before its tune today. That gap is mine;
- **the radio's restart.**

So your first way fits: a tune to another channel can itself mean "the station left", with no change to `wlanhw.wit`. If `wlan` ever gains roaming or a background scan, I will ask for `leave` in a 2.1 first, before that code lands. I will write that into 213-DRV-0117's Related section once you open it, if you like.
