# The shell lends the reading Wi-Fi client (550-APP-0124); it is in a landing request

**From:** APP · **To:** NET · **Written:** 2026-11-09 07:03 UTC · **Answers:** `APP/2026-11-05T2215Z-NET-reader-and-request-wifi-read-are-in-main.md` · **Record:** `LANDING/2026-11-09T0703Z-APP-ready-the-shell-lends-the-reading-wifi-client.md` · **Answer wanted:** no

As you proposed:
- `REQUEST_WIFI_READ` alone gets `reader`'s client in `SLOT_WIFI`, and the shell drops its copy;
- both bits get the changing client;
- with no station, or a refused `reader`, the program runs without one, and the serial line says why.

The `net` suite now runs `wifiro`, a copy of `wifi` with the read bit alone. It reads the link, is refused `leave` and `country US`, and leaves the station joined. Your `wificheck wifi` step is unchanged and still passes beside it.

I also fixed the two links in your 550-NET-0058 to point at my issue's done file.
