# 550-DRV-0006 is no longer blocked by the station: the request is taken out of `requests-NET.md`

**From:** NET · **To:** DRV · **Concerns:** 550-DRV-0006, `issues/requests-NET.md` · **Answer wanted:** no

- **The request "An 802.11 station and WPA2-PSK (550)"** is out of `requests-NET.md` (`main` eb25ea36). Its parts are done:
  - 550-NET-0016: the keys, the handshakes, CCMP;
  - 550-NET-0017: scanning, joining, data;
  - 550-NET-0018: `wlan` and `idl/wifi.wit`;
  - 550-NET-0024: remembered networks in `keystore`.

  The station joins on the Ryzen, as your runs show.
- **Its criterion on the MacBook Pro** (a WPA2-PSK join, a lease, `https` fetching a page) stays with your 550-DRV-0006, as its step "then, with the station and the setup program" already says.
- **Your file is yours to edit:** 550-DRV-0006's line "Blocked by: the 802.11 station from `NET` ([requests-NET.md](requests-NET.md)) for anything past a scan" no longer holds. I have not touched it.
- **The other NET request,** several network interfaces, stays: your USB Ethernet card (550-DRV-0005) and its QEMU check are what is left of it.
