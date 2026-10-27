# RTL8821CE: what rtw88_wifi ports from Linux rtw88 v6.12

Task: [550-DRV-0042](../issues/550-DRV-0042-rtl8821ce-wifi.md). The driver is `rtw88_wifi`. Its tables come from `scripts/hardware/rtw88_tables.py`, and its firmware from `scripts/firmware/rtl8821ce.py`. The licence and the sources are recorded in [THIRD_PARTY.md](../THIRD_PARTY.md) and [LICENSES/BSD-3-Clause-rtw88.txt](../LICENSES/BSD-3-Clause-rtw88.txt).

The specification below was written before the code. It follows rtw88's code paths, resolved for the 8821C on PCIe, and cites `file:line` in Linux v6.12 (`adc218676eef`). Statements marked **[vendor]** come from Realtek's out-of-tree halmac headers. Those marked **[inferred]** are a reading, not code facts. Nothing in it has been run on a chip until the DEXP's results are recorded in the task (MC-12.1, MC-12.3).

---


Source: Linux v6.12 `drivers/net/wireless/realtek/rtw88/` (GPL-2.0 OR BSD-3-Clause, taken under BSD-3-Clause).
All `file:line` references are to the files in this directory. Everything below is resolved for
`rtw8821c_hw_spec` (rtw8821c.c:1989-2070) + PCI HCI (`rtw_pci_ops`, pci.c:1594-1614).
Statements marked **[vendor]** come from Realtek's out-of-tree halmac headers
(github.com/tomaspinho/rtl8821ce, `hal/halmac/*.h`, copies in `../vendor/`), not from rtw88; they are
not exercised by Linux rtw88 and must be validated on hardware. Statements marked **[inferred]** are my
reading, not code facts.

Conventions: `R8/R16/R32(a)` = MMIO read of width at BAR offset `a`; `W8/W16/W32(a,v)`;
`SET8(a,m)` = `W8(a, R8(a)|m)`; `CLR8(a,m)` = `W8(a, R8(a)&~m)` (same for 16/32);
`MASK32(a,m,v)` = `W32(a, (R32(a)&~m) | ((v<<ffs(m)) & m))` (hci.h:233-245);
`MASK8` same at byte width (hci.h:247-259). All registers little-endian. RF register writes/reads: see §6.1.

---------------------------------------------------------------------------------------------------

## 0. Chip constants resolved (rtw8821c.c:1989-2070)

| field | value | consequence |
|---|---|---|
| `id` | `RTW_CHIP_TYPE_8821C` | 8821C-only branches: DEVCTL2 completion-timeout disable (pci.c:1496-1503), H2C 0xD1 after FW DL (mac.c:991-993), Intel-bridge rx_no_aspm (pci.c:1767-1769) |
| `wlan_cpu` | `RTW_WCPU_11AC` | every `rtw_chip_wcpu_11ac()` branch taken: DDMA FW download (not legacy page DL), H2C TX ring + HIMR3/HISR3, `__rtw_mac_init_system_cfg`, `__priority_queue_cfg` (not `_legacy`), `init_h2c`, RPWM toggle in power switch, 80M MAC clock in `rtw_set_channel_mac` |
| `fw_name` | `rtw88/rtw8821c_fw.bin` | linux-firmware |
| `tx_pkt_desc_sz` / `tx_buf_desc_sz` | 48 / 16 | TX descriptor 48 B; TX BD ring entry 16 B (2 segments × 8 B) |
| `rx_pkt_desc_sz` / `rx_buf_desc_sz` | 24 / 8 | RX descriptor 24 B; RX BD ring entry 8 B |
| `phy_efuse_size` / `log_efuse_size` / `ptct_efuse_size` | 512 / 512 / 96 | physical map parsed over bytes 0..415 |
| `txff_size` / `rxff_size` | 65536 / 16384 | 512 TX pages of 128 B |
| `rsvd_drv_pg_num` | 8 | |
| `page_size` | 128 (`TX_PAGE_SIZE`, main.h:34-35) | |
| `csi_buf_pg_num` | 0 | |
| `sys_func_en` | 0xD8 | |
| `max_power_index` | 0x3F | TX AGC index is 6-bit |
| `txgi_factor` | 1 | |
| `is_pwr_by_rate_dec` | true | by-rate table bytes are BCD |
| `rf_base_addr` / `rf_sipi_addr` | {0x2800,0x2c00} / {0xc90,0xe90} | path A only used |
| `rf_tbl` | `{&rtw8821c_rf_a_tbl}` | 1 RF path |
| `dig` | `{0xc50, 0x7f}` | IGI path A |
| `ltecoex_addr` | ctrl 0x1700, wdata 0x1704, rdata 0x1708 | |
| `ops.read_rf` / `ops.write_rf` | `rtw_phy_read_rf` (direct) / `rtw_phy_write_rf_reg_sipi` | §6.1 |
| `ops.efuse_grant`, `adaptivity_init`, `cfo_init`, `config_txrx_mode`, `dpk_track`, `shutdown` | NULL | no-ops |
| `rfk_init_tbl`, `edcca_th`, `wow_fw_name` | NULL | no RFK table, no EDCCA, no WoW FW |
| `rfe_defs` | [0],[2],[4],[6] only (rtw8821c.c:1625-1630) | **RFE types 1,3,5,≥7 are rejected by Linux (-ENODEV, phy.h:131-142)** |
| `scbd_support` | true | BT scoreboard 0xAA used |

HCI-derived constants (main.c:1824-1828): `rpwm_addr = 0x3D9`, `cpwm_addr = 0x3DA`.

### 0.1 Answers for the `info` call
- MAC address: logical efuse 0xD0..0xD5 (§5.3); invalid → Linux generates a random one (main.c:2011-2014).
- Bands: 2.4 GHz + 5 GHz (`chip->band`). Channels as registered by Linux (main.c:45-89): 2.4 GHz 1–14 (14 = CCK-only
  Japan; needs special CCK filter, §7.1); 5 GHz 36,40,44,48, 52–64 (step 4), 100–144 (step 4), 149,153,157,161,165.
  52–144 are DFS: no radar detection here ⇒ RX/passive only.
- Max frame: RX buffers hold an 11454-byte MPDU (`RTK_PCI_RX_BUF_SIZE`, pci.h:14-15), `REG_RX_PKT_LIMIT` = 12288 B;
  TX length field is 16-bit. Without A-MSDU/A-MPDU report 2346 B (non-HT MPDU max) [inferred choice].
- Current channel: driver state set by `set-channel` (Linux keeps it in `hal.current_channel`, initial 1, main.c:2135).

---------------------------------------------------------------------------------------------------

## 1. PCI bring-up

### 1.1 What Linux does (pci.c:1708-1801, probe order)
1. `pci_enable_device` + `pci_set_master` (pci.c:1536-1551): PCI Command register: Memory Space + Bus Master.
2. `pci_request_regions`; map **BAR index 2** (`bar_id = 2`, pci.c:1204), full length (pci.c:1213-1214).
   [inferred] On 8821CE BAR0 is I/O (unused) and BAR2 is a 64-bit non-prefetchable MMIO BAR
   (config 0x18/0x1C), 64 KiB. Highest offset touched by the driver is 0xFF1A (coex, §6.7), so 64 KiB is needed.
3. Ring allocation (`rtw_pci_init`, pci.c:368-392 → §8).
4. `rtw_chip_info_setup` (power on, FW, efuse, power off — §2–§5).
5. `rtw_pci_phy_cfg` (pci.c:1451-1507), run *after* step 4 (MAC powered off at that point):
   - PCIe Gen1 PHY parameter via MDIO: one entry `{offset 0x09, value 0x6380, IP_SEL_PHY, CUT_ALL}`
     (rtw8821c.c:1596-1605); Gen2 table is empty (terminator only). `rtw_mdio_write(addr=0x09, data=0x6380, g1=true)`
     (pci.c:1283-1307):
     ```
     W16(0x3F4, 0x6380)                 // REG_MDIO_V1
     W8 (0x3F8, 0x09 & 0x1F)            // REG_PCIE_MIX_CFG: address
     W8 (0x3FB, 0)                      // page = (addr<0x20 ? 0 : 1) + (g1 ? 0 : 2) = 0
     MASK32(0x3F8, BIT5, 1)             // BIT_MDIO_WFLAG_V1: start
     poll R32(0x3F8) bit5 == 0, 20 tries × 10 µs
     ```
   - `rtw_pci_link_cfg` (pci.c:1394-1434): only if the host set CLKREQ_EN in the device's PCIe Link Control:
     DBI read byte 0x719, set BIT4 (CLKREQ_SW_EN), DBI write. **Skippable** (chip-side CLKREQ module defaults to
     disabled, comment pci.c:1408-1423).
   - **8821C only:** `pcie_capability_set_word(PCI_EXP_DEVCTL2, PCI_EXP_DEVCTL2_COMP_TMOUT_DIS)` —
     set bit 4 (0x0010) of Device Control 2 = PCIe capability offset +0x28 (pci.c:1496-1503). **Do this.**
6. IRQ: `pci_alloc_irq_vectors(INTX|MSI)` + threaded handler (pci.c:1616-1640). Not needed for polling.

DBI access (only for CLKREQ/ASPM, pci.c:1235-1281): write: `W8(0x3E8 + (addr&3), data)`;
`W16(0x3F0, (addr & 0xFFC) | (BIT(addr&3) << 12))`; `W8(0x3F2, 0x01)`; poll `R8(0x3F2)==0` (20×10 µs).
Read: `W16(0x3F0, addr & 0xFFC)`; `W8(0x3F2, 0x02)`; poll `R8(0x3F2)==0`; data = `R8(0x3EC + (addr&3))`.

### 1.2 ASPM / CLKREQ
Linux enables chip ASPM L1 (DBI 0x719 BIT3, `BIT_L1_SW_EN`) only while in LPS (`rtw_pci_link_ps`, pci.c:1372-1392) and
only if the host enabled L1 in LNKCTL. With no LPS it is never set. On Intel upstream bridges Linux additionally
disables it around RX polling (pci.c:1767-1769, 1655-1682). **Minimal driver: never touch 0x719; optionally clear
ASPM bits (LNKCTL[1:0]) in the device and its upstream port for robustness [inferred].**

### 1.3 DMA addressing
rtw88 never calls `dma_set_mask`, so all DMA is 32-bit. All BD `dma` fields are `__le32` (pci.h:165-197) and
ring base registers are written with `rtw_write32` only (pci.c:405-463). **Every ring and every TX/RX buffer must be
below 4 GiB.** [inferred] Base registers are 8 bytes apart (0x310,0x318,…), the +4 dword is presumably the high
half; leave it 0.

### 1.4 Interrupt registers (pci.h:81-145)
| reg | addr | Linux mask (pci.c:373-386) |
|---|---|---|
| HIMR0 / HISR0 | 0x0B0 / 0x0B4 | 0x000044FD = HIGHDOK b7, MGNTDOK b6, BKDOK b5, BEDOK b4, VIDOK b3, VODOK b2, ROK b0, BCNDMAINT_E b14, C2HCMD b10 |
| HIMR1 / HISR1 | 0x0B8 / 0x0BC | 0x00000200 = TXFOVW b9 |
| HIMR3 / HISR3 | 0x10B8 / 0x10BC | 0x00010000 = H2CDOK b16 (11ac chips only) |

Enable: write the three HIMRs (pci.c:479-495). ISR (pci.c:1118-1197): disable all HIMR, read HISRx & mask, write the
same value back to HISRx (W1C), service, re-enable. `IMR_C2HCMD` is only the firmware-crash trigger
(`R8(0x84)==1` → recovery, fw.c:375-381); normal C2H messages arrive in the RX ring (§10.3).

### 1.5 Polling without interrupts — workable
Nothing in the data path depends on HISR: Linux NAPI itself counts RX by reading the HW write index
(`rtw_pci_get_hw_rx_ring_nr`, pci.c:1022-1038) and TX completion by reading the HW read index
(`rtw_pci_tx_isr`, pci.c:954-962). Keep HIMR0/1/3 = 0 and poll:
RX `R32(0x3B4)[27:16]`, TX `R32(TXBD_IDX_q)[27:16]`. C2H/TX reports come through the RX ring. Poll RX often enough
that the ring (Linux: 512 entries) never fills.

---------------------------------------------------------------------------------------------------

## 2. Power on

### 2.1 Linux call flow
Probe: `rtw_chip_info_setup` (main.c:2043) → `rtw_chip_parameter_setup` (main.c:1818-1869) →
`rtw_chip_efuse_info_setup` (main.c:1958-2022): `rtw_chip_efuse_enable` = `rtw_hci_setup` + `rtw_mac_power_on` +
`W8(0x1A0, 0xFD)` (C2H_HW_FEATURE_DUMP) + `rtw_download_firmware`; then efuse parse, HW-feature dump, then
`rtw_chip_efuse_disable` = `rtw_hci_stop` + `rtw_mac_power_off`.
Start (`rtw_ops_start` → `rtw_core_start`, main.c:1484-1506 → `rtw_power_on`, main.c:1353-1415):
```
rtw_hci_setup            (pci: ring regs + DMA reset, §8.2)
rtw_mac_power_on         (§2.3)
rtw_download_firmware    (§4)
rtw_mac_init             (§3)
chip->ops->phy_set_param = rtw8821c_phy_set_param (§6)
rtw_hci_start            (pci: enable IRQs)
rtw_fw_send_general_info, rtw_fw_send_phydm_info   (§12)
rtw_coex_power_on_setting; rtw_coex_init_hw_config(wifi_only = !efuse.btcoex)   (§6.7)
```
then `rtw_sec_enable_sec_engine` (skip, §11.4), `W32(REG_RCR 0x608, hal.rcr)` (§11).

### 2.2 Before anything: chip parameters (main.c:1842-1866)
`v = R32(0xF0)` (REG_SYS_CFG1): `cut_version = (v>>12)&0xF`; `mp_chip = !(v & BIT23)`;
`rf_path_num = (v & BIT27) ? 2 : 1` — 8821C must read 1 (1T1R). `antenna_tx = antenna_rx = BB_PATH_A (1)`.
`hal.rcr` starts as 0xF010400E (main.c:2141-2143) and gets `|= BIT26` (VHT_DACK) → **0xF410400E**.

### 2.3 `rtw_mac_power_on` (mac.c:376-408), resolved for PCIe/11ac
```
pre_system_cfg()                         // mac.c:62-137
r = power_switch(on)                     // mac.c:271-326
if r == ALREADY_ON: power_switch(off); pre_system_cfg(); power_switch(on)
init_system_cfg()                        // mac.c:328-351
```
**pre_system_cfg (PCIe, 11ac):**
```
W8(0x1C, 0)                              // REG_RSV_CTRL
SET32(0x74, BIT8)                        // REG_HCI_OPT_CTRL |= BIT_USB_SUS_DIS (yes, also on PCIe)
SET32(0x64, BIT29|BIT28)                 // REG_PAD_CTRL1 |= PAPE_WLBT_SEL|LNAON_WLBT_SEL
CLR32(0x4C, BIT25|BIT26)                 // REG_LED_CFG &= ~(PAPE_SEL_EN|LNAON_SEL_EN)
SET32(0x40, BIT2)                        // REG_GPIO_MUXCFG |= WLRFE_4_5_EN
CLR8 (0x02, BIT0|BIT1)                   // REG_SYS_FUNC_EN: BB_RSTB, BB_GLB_RST off
CLR8 (0x1F, BIT2|BIT1|BIT0)              // REG_RF_CTRL: RF_SDM_RSTB|RF_RSTB|RF_EN off
CLR32(0xEC, BIT24|BIT25|BIT26)           // REG_WLRF1 BBRF_EN off
```
**power_switch(on/off)** (mac.c:271-326):
```
rpwm = R8(0x3D9)
if R16(0x80) == 0xC078:                  // FW still running (FW_READY pattern)
    W8(0x3D9, (rpwm ^ BIT7) & BIT7)      // toggle RPWM
cur_on = (R8(0x100) != 0xEA)             // REG_CR reads 0xEA when MAC is off
if want == cur_on: return ALREADY
run card_enable_flow (on) or card_disable_flow (off)
```
**Power sequence interpreter** (mac.c:139-269): for each table entry with `(intf_mask & PCI(BIT2))` and
`(cut_mask & (1 << (cut+1)))` (all 8821C entries are CUT_ALL):
WRITE: `W8(off, (R8(off) & ~mask) | (value & mask))`;
POLL: read8 every 50 µs up to 20000 times (1 s) until `(R8 & mask) == (value & mask)`; on PCIe failure toggle
`REG_SYS_PW_CTRL 0x04` BIT3: `v=R8(0x04); W8(0x04, v|BIT3); W8(0x04, v&~BIT3)` and poll once more;
DELAY: none for PCIe in 8821C tables.

**card_enable_flow_8821c** = `trans_carddis_to_cardemu_8821c` + `trans_cardemu_to_act_8821c`
(rtw8821c.c:1257-1397, 1570-1574), PCIe-applicable entries in order (offset, mask, value):
| # | op | offset | mask | value | src line |
|---|---|---|---|---|---|
| 1 | W | 0x0005 | 0x98 (b3,b4,b7) | 0x00 | 1273-1277 |
| 2 | W | 0x0300 | 0xFF | 0x00 | 1278-1282 (PCI) |
| 3 | W | 0x0301 | 0xFF | 0x00 | 1283-1287 (PCI) |
| 4 | W | 0x0005 | 0x1C (b4,b3,b2) | 0x00 | 1311-1315 |
| 5 | W | 0x0075 | 0x01 | 0x01 | 1316-1320 (PCI) |
| 6 | POLL | 0x0006 | 0x02 | 0x02 | 1321-1325 |
| 7 | W | 0x0075 | 0x01 | 0x00 | 1326-1330 (PCI) |
| 8 | W | 0x0006 | 0x01 | 0x01 | 1331-1335 |
| 9 | W | 0x0005 | 0x80 | 0x00 | 1336-1340 |
| 10 | W | 0x0005 | 0x18 | 0x00 | 1341-1345 |
| 11 | W | 0x0005 | 0x01 | 0x01 | 1351-1355 (power-on request) |
| 12 | POLL | 0x0005 | 0x01 | 0x00 | 1356-1360 (auto-clears when done) |
| 13 | W | 0x0020 | 0x08 | 0x08 | 1361-1365 |
| 14 | W | 0x0074 | 0x20 | 0x20 | 1366-1370 (PCI) |
| 15 | W | 0x0022 | 0x02 | 0x00 | 1371-1375 (PCI) |
| 16 | W | 0x0062 | 0xE0 | 0xE0 | 1376-1381 (PCI) |
| 17 | W | 0x0061 | 0xE0 | 0x00 | 1382-1386 (PCI) |
| 18 | W | 0x007C | 0x02 | 0x00 | 1387-1391 |
(Skipped for PCIe: SDIO 0x86 entries, USB 0x4A, 0x20/0x01-delay/0x00 USB|SDIO, 0x10C3 USB.)

**card_disable_flow_8821c** = `trans_act_to_cardemu_8821c` + `trans_cardemu_to_carddis_8821c`
(rtw8821c.c:1399-1568, 1576-1580), PCIe entries:
| # | op | offset | mask | value |
|---|---|---|---|---|
| 1 | W | 0x0093 | 0x08 | 0x00 |
| 2 | W | 0x001F | 0xFF | 0x00 |
| 3 | W | 0x0049 | 0x02 | 0x00 |
| 4 | W | 0x0006 | 0x01 | 0x01 |
| 5 | W | 0x0002 | 0x02 | 0x00 |
| 6 | W | 0x0005 | 0x02 | 0x02 |
| 7 | POLL | 0x0005 | 0x02 | 0x00 |
| 8 | W | 0x0020 | 0x08 | 0x00 |
| 9 | W | 0x0067 | 0x20 | 0x00 |
| 10 | W | 0x0005 | 0x04 | 0x04 (PCI) |
| 11 | W | 0x0081 | 0xC0 | 0x00 |
| 12 | W | 0x0090 | 0x02 | 0x00 (USB|PCI) |

**init_system_cfg (11ac)** (mac.c:328-351):
```
SET32(0x1080, BIT16|BIT8)                // REG_CPU_DMEM_CON |= WL_PLATFORM_RST|DDMA_EN
SET8 (0x03, 0xD8)                        // REG_SYS_FUNC_EN+1 |= chip->sys_func_en
W8(0x1103, (R8(0x1103) & 0xF0) | 0x0C)   // REG_CR_EXT+3
if R32(0x80) & BIT20:                    // BIT_BOOT_FSPI_EN
    W32(0x80, R32(0x80) & ~BIT20); W32(0x40, R32(0x40) & ~BIT19)
```
Power off (`rtw_power_off`, main.c:1508-1513): `rtw_hci_stop` (disable HIMR, reprogram rings §8.2),
`W16(0xAA, 0x8000)` (coex power-off scoreboard, coex.c:2751-2754), `power_switch(off)`.

---------------------------------------------------------------------------------------------------

## 3. MAC init (after firmware download)

`rtw_mac_init` (mac.c:1382-1402) = `rtw_init_trx_cfg` (txdma_queue_mapping, priority_queue_cfg, init_h2c) →
`rtw8821c_mac_init` → `rtw_drv_info_cfg` → `rtw_pci_interface_cfg` (no-op for 8821C, pci.c:1436-1449).

### 3.1 txdma_queue_mapping (mac.c:1079-1128), PCIe uses `rqpn_table_8821c[1]` (rtw8821c.c:1656-1658)
vo=NORMAL(2) vi=NORMAL(2) be=LOW(1) bk=LOW(1) mg=EXTRA(0) hi=HIGH(3):
```
W16(0x10C, 0xC5A0)    // REG_TXDMA_PQ_MAP: HIQ[15:14]=3 MGQ[13:12]=0 BKQ[11:10]=1 BEQ[9:8]=1 VIQ[7:6]=2 VOQ[5:4]=2
W8(0x100, 0x00)
W8(0x100, 0xFF)       // REG_CR = MAC_TRX_ENABLE (HCI TX/RX DMA, TX/RX DMA, protocol, schedule, MAC TX/RX)
W32(0x1330, BIT31)    // REG_H2CQ_CSR = H2CQ_FULL
```
### 3.2 set_trx_fifo_info (mac.c:1130-1181), computed for 8821C
txff_pg_num = 65536>>7 = **512**; rsvd_pg_num = 8 (drv) + 24 (H2C extra) + 8 (H2C static) + 8 (H2CQ) + 0 (CPU instr)
+ 4 (FW txbuf) + 0 (CSI) = **52** (mac.h:24-29); acq_pg_num = rsvd_boundary = **460**.
Page addresses (from top): csibuf 512, fw_txbuf **508**, cpu_instr 508, h2cq **500**, h2c_sta_info 492,
h2c_info 468, drv **460** (= boundary, checked).

### 3.3 priority_queue_cfg (mac.c:1183-1221, 1251-1290), PCIe uses `page_table_8821c[1]` = {hq 16, nq 16, lq 16, exq 14, gap 1}
pubq = 460 − 16 − 16 − 16 − 14 − 1 = **397**.
```
W16(0x230, 16)   // REG_FIFOPAGE_INFO_1 = HPQ
W16(0x234, 16)   // INFO_2 = LPQ
W16(0x238, 16)   // INFO_3 = NPQ
W16(0x23C, 14)   // INFO_4 = EXQ
W16(0x240, 397)  // INFO_5 = PUBQ
SET32(0x22C, BIT31)              // REG_RQPN_CTRL_2 |= BIT_LD_RQPN
W16(0x204, 460)                  // REG_FIFOPAGE_CTRL_2 = rsvd boundary
SET8(0x422, BIT4)                // REG_FWHW_TXQ_CTRL+2 |= BIT_EN_WR_FREE_TAIL>>16
W16(0x424, 460)                  // REG_BCNQ_BDNY_V1
W16(0x206, 460)                  // REG_FIFOPAGE_CTRL_2+2
W16(0x456, 460)                  // REG_BCNQ1_BDNY_V1
W32(0x11C, 16384-256-1 = 0x3EFF) // REG_RXFF_BNDY (C2H_PKT_BUF=256 reserved)
SET8(0x208, BIT0)                // REG_AUTO_LLT_V1 |= AUTO_INIT_LLT
poll R32(0x208) bit0 == 0        // check_hw_ready: 1000 × 10 µs (util.c:9-21)
W8(0x103, 0)                     // REG_CR+3
```
### 3.4 init_h2c (mac.c:1292-1343)
h2cq_addr = 500<<7 = **0xFA00**, h2cq_size = 8<<7 = **0x400**:
```
W32(0x244, (R32(0x244) & 0xFFFC0000) | 0xFA00)   // REG_H2C_HEAD
W32(0x24C, (R32(0x24C) & 0xFFFC0000) | 0xFA00)   // REG_H2C_READ_ADDR
W32(0x248, (R32(0x248) & 0xFFFC0000) | 0xFE00)   // REG_H2C_TAIL = addr+size
W8(0x254, (R8(0x254) & 0xFC) | 0x01)             // REG_H2C_INFO
W8(0x254, (R8(0x254) & 0xFB) | 0x04)
W8(0x20D, (R8(0x20D) & 0x7F) | 0x80)             // REG_TXDMA_OFFSET_CHK+1
wp = R32(0x10D4) & 0x3FFFF; rp = R32(0x10D0) & 0x3FFFF
free = wp >= rp ? 0x400 - (wp - rp) : rp - wp ;  must equal 0x400 else fail
```
### 3.5 rtw8821c_mac_init (rtw8821c.c:207-265; constants rtw8821c.h:123-175)
```
W8 (0x455, 0x70)                 // REG_AMPDU_MAX_TIME_V1
SET8(0x45E, BIT2)                // REG_TX_HANG_CTRL |= EN_EOF_V1
W8 (0x4E5, 0xE4); W8(0x4E6, 0x09)// REG_PRECNT_CTRL = 0x1E4 | BIT11
W32(0x4C8, 0x202008FF)           // REG_PROT_MODE_CTRL: RTS len 0xFF, RTS time 8, agg 0x20, RTS agg 0x20
W16(0x4CE, 0x0801)               // REG_BAR_MODE_CTRL+2: BAR retry 1, RA try agg 8
W8 (0x1448, 6); W8(0x144A, 6)    // FAST_EDCA VO/VI
W8 (0x144C, 6); W8(0x144E, 6)    // FAST_EDCA BE/BK
SET8(0x480, BIT5)                // REG_INIRTS_RATE_SEL
CLR8(0x5B4, BIT4|BIT5|BIT6)      // REG_TIMER0_SRC_SEL
W16(0x522, 0)                    // REG_TXPAUSE = 0 (no queue paused)
W8 (0x51B, 0x09)                 // REG_SLOT
W8 (0x512, 0x19)                 // REG_PIFS
W32(0x514, 0x10100E0A)           // REG_SIFS: CCK ctx 0x0A, OFDM ctx 0x0E, CCK trx 0x10, OFDM trx 0x10
W16(0x502, 0x0186)               // VO TXOP
W16(0x506, 0x03BC)               // VI TXOP
W32(0x544, 0x001B0005)           // REG_RD_NAV_NXT
W16(0x55E, 0x3030)               // REG_RXTSF_OFFSET_CCK/OFDM
SET8(0x550, BIT3)                // REG_BCN_CTRL |= EN_BCN_FUNCTION
W32(0x540, 0x00006404)           // REG_TBTT_PROHIBIT
W8 (0x558, 0x04)                 // REG_DRVERLYINT
W8 (0x559, 0x02)                 // REG_BCNDMATIM
CLR8(0x521, BIT4)                // REG_TX_PTCL_CTRL+1 &= ~(SIFS_BK_EN>>8)
W32(0x6A0, 0x0FFFFFFF)           // REG_RXFLTMAP0 (mgmt all) + RXFLTMAP1 (ctrl, subtypes 12..15 off)
W16(0x6A4, 0xFFFF)               // REG_RXFLTMAP2 (data all)
W32(0x608, 0xE400220E)           // REG_RCR (overwritten later)
W8 (0x60C, 0x18)                 // REG_RX_PKT_LIMIT = 12288>>9
W8 (0x606, 0x30); W8(0x605, 0x30)// REG_TCR+2 / +1
W8 (0x639, 0x40)                 // REG_ACKTO_CCK
SET8(0x66C, BIT1)                // REG_WMAC_TRXPTCL_CTL_H
SET8(0x718, BIT6)                // REG_SND_PTCL_CTRL |= DIS_CHK_VHTSIGB_CRC
W32(0x7D8, 0xB0810041)           // REG_WMAC_OPTION_FUNCTION+8
W8 (0x7D4, 0x98)                 // REG_WMAC_OPTION_FUNCTION+4
```
### 3.6 rtw_drv_info_cfg (mac.c:1364-1380)
```
W8(0x60F, 4)                       // REG_RX_DRVINFO_SZ = PHY_STATUS_SIZE (×8 B = 32 B)
W8(0x115, (R8(0x115) & 0xF0) | 0xF)// REG_TRXFF_BNDY+1, "rxdesc len = 0 issue"
SET32(0x608, BIT28)                // RCR |= APP_PHYSTS
CLR32(0x7D4, BIT8|BIT9)
```
Note (mac.c:998-1077): prio-queue page accounting registers for flushing: INFO_1=HIGH, INFO_2=LOW, INFO_3=NORMAL,
INFO_4=EXTRA (rsvd at +0, avail at +2, 16-bit; rtw8821c.c:1670-1684). Not needed for a minimal driver.

---------------------------------------------------------------------------------------------------

## 4. Firmware

### 4.1 File and header (`struct rtw_fw_hdr`, fw.h:303-330; FW_HDR_SIZE 64, FW_HDR_CHKSUM_SIZE 8, fw.h:12-13)
| off | size | field |
|---|---|---|
| 0x00 | 2 | signature |
| 0x02 | 1 | category |
| 0x03 | 1 | function |
| 0x04 | 2 | version |
| 0x06 | 1 | subversion |
| 0x07 | 1 | subindex |
| 0x08 | 4 | rsvd |
| 0x0C | 4 | feature (valid only if bit0 `FW_FEATURE_SIG` set; main.c:1712-1725) |
| 0x10..0x13 | 4 | month, day, hour, min |
| 0x14 | 2 | year |
| 0x16 | 2 | rsvd3 |
| 0x18 | 1 | mem_usage (**bit4 = EMEM present**) |
| 0x19 | 3 | rsvd4 |
| 0x1C | 2 | h2c_fmt_ver |
| 0x1E | 2 | rsvd5 |
| 0x20 | 4 | dmem_addr (bit31 must be cleared before use) |
| 0x24 | 4 | dmem_size |
| 0x28, 0x2C | 4+4 | rsvd6/7 |
| 0x30 | 4 | imem_size |
| 0x34 | 4 | emem_size |
| 0x38 | 4 | emem_addr |
| 0x3C | 4 | imem_addr |
File layout: `[hdr 64][DMEM dmem_size][8 B checksum][IMEM imem_size][8 B][EMEM emem_size][8 B]` (EMEM part only if
mem_usage bit4). Validation (mac.c:415-436): `64 + (dmem+8) + (imem+8) + (emem ? emem+8 : 0) == file size`.
The 8-byte checksums are downloaded together with each section.

### 4.2 Download method for 8821C: **DDMA via reserved page (11ac path), not legacy page writes**
`rtw_download_firmware` → `_rtw_download_firmware` → `__rtw_download_firmware` (mac.c:774-829, 974-996).
Prerequisite: PCI rings programmed (`rtw_pci_setup`) and MAC powered on.
```
1  check_firmware_size()
2  lte_bckp = LTE_indirect_read(0x38)                // util.c:24-36, §6.7
3  wlan_cpu_enable(false): CLR8(0x03, BIT2) /*FEN_CPUEN*/; CLR8(0x1D, BIT0) /*WLMCU_IOIF*/   // mac.c:438-453
4  backup & set (mac.c:457-509), save the 6 values for step 7:
     b0 = R8(0x10D);  W8(0x10D, 0xC0)               // TXDMA_PQ_MAP+1: HIQ → HIGH
     b1 = R8(0x100);  W8(0x100, 0x05)               // REG_CR = HCI_TXDMA_EN|TXDMA_EN
     b2 = 0x80000000 (constant); W32(0x1330, 0x80000000)   // H2CQ_CSR = FULL
     b3 = R16(0x230); W16(0x230, 0x200)             // HPQ = 512 pages
     b4 = R32(0x22C)|BIT31; W32(0x22C, b4)          // load RQPN
     b5 = R8(0x550);  W8(0x550, (b5 & ~BIT3) | BIT4) // BCN_CTRL: no BCN function, DIS_TSF_UDT
5  reset platform (mac.c:511-517):
     CLR8(0x1082, BIT0); CLR8(0x09, BIT6); SET8(0x1082, BIT0); SET8(0x09, BIT6)
       // REG_CPU_DMEM_CON+2 WL_PLATFORM_RST, REG_SYS_CLK_CTRL+1 CPU_CLK_EN
6  start_download_firmware (mac.c:694-743):
     W16(0x80, (R16(0x80) & 0x3800) | BIT0)        // REG_MCUFW_CTRL: clear status, MCUFWDL_EN
     download_to_mem(src=hdr+64,               dst=dmem_addr&~BIT31, len=dmem_size+8)
     download_to_mem(src=…+dmem_size+8,        dst=imem_addr&~BIT31, len=imem_size+8)
     if emem: download_to_mem(src=…,           dst=emem_addr&~BIT31, len=emem_size+8)
7  restore: W8(0x10D,b0); W8(0x100,b1); W32(0x1330,b2); W16(0x230,b3); W32(0x22C,b4); W8(0x550,b5)
8  end flow (mac.c:759-772): W32(0x210, BIT2)  /*TXDMA_STATUS: clear PAGE_OVF*/;
     v = R16(0x80); if (v & 0x50) == 0x50 /*IMEM+DMEM CHKSUM_OK*/: W16(0x80, (v | BIT14) & ~BIT0)
9  wlan_cpu_enable(true): SET8(0x1D, BIT0); SET8(0x03, BIT2)
10 LTE_indirect_write(0x38, lte_bckp)
11 validate (mac.c:745-757): poll (R32(0x80) & 0xFFFF) == 0xC078   // FW_READY = FW_INIT_RDY b15 |
       FW_DW_RDY b14 | IMEM_DW_OK b3 | IMEM_CHKSUM_OK b4 | DMEM_DW_OK b5 | DMEM_CHKSUM_OK b6
     Linux: 1000 × 10 µs. On failure: (R32(0x10FC) & 0xFFFFFF00) == 0xFAAAAA00 → "invalid fw key".
12 rtw_hci_setup again (§8.2: reprogram all rings, clear indices, DMA reset)
13 h2c mailbox index = 0, H2C packet seq = 0
14 PCIe + 8821C: H2C mailbox cmd 0xD1 RECOVER_BT_DEV, byte1 bit0 = 1 (mac.c:991-993, fw.c:968-976)
on any failure: CLR8(0x80, BIT0); SET8(0x03, BIT2)       // mac.c:823-828
```
**download_to_mem(data, dst, size)** (mac.c:643-692), chunks of max **0x1000** bytes:
```
SET32(0x1208, BIT25)                        // DDMA_CH0CTRL |= RESET_CHKSUM_STS
first = 1; off = 0
while remaining:
    n = min(remaining, 0x1000)
    write_rsvd_page(pg_addr = 0, data+off, n)        // §4.3: TX desc+chunk into TX FIFO page 0
    poll R32(0x1208) bit31 (OWN) == 0                // 1000 × 10 µs
    W32(0x1200, 0x18780000 + 0 + 48)                 // DDMA_CH0SA = OCPBASE_TXBUF + page0 + TX desc size
    W32(0x1204, dst + off)                           // DDMA_CH0DA
    W32(0x1208, BIT31 | BIT29 | (n & 0x3FFFF) | (first ? 0 : BIT24))  // OWN|CHKSUM_EN|len|CHKSUM_CONT
    poll R32(0x1208) bit31 == 0
    first = 0; off += n
// checksum check (mac.c:609-641):
c = R8(0x80)
if R32(0x1208) & BIT27 (CHKSUM_STS):  // error
    if dst < 0x200000: W8(0x80, (c | BIT3) & ~BIT4) else W8(0x80, (c | BIT5) & ~BIT6); fail
else if dst < 0x200000 (OCPBASE_DMEM_88XX): W8(0x80, c | BIT3 | BIT4)   // IMEM ok
else W8(0x80, c | BIT5 | BIT6)                                              // DMEM ok
```
### 4.3 write_rsvd_page(pg_addr, buf, n) (fw.c:1443-1506 + pci.c:868-892, 798-866)
```
W16(0x204, (pg_addr & 0xFFF) | BIT15)          // REG_FIFOPAGE_CTRL_2: BCN head + BCN_VALID_V1 (W1C)
cr1 = R8(0x101); W8(0x101, cr1 | BIT0)         // REG_CR+1 |= ENSWBCN>>8
tq  = R8(0x422); W8(0x422, tq & ~BIT6)         // REG_FWHW_TXQ_CTRL+2 &= ~(EN_BCNQ_DL>>16)
build TX desc (48 B) + payload n in one DMA buffer (<4 GiB) — desc fields §9.3 "Reserved-page/FW chunk"
BCN ring has exactly 1 BD (pci.h:247-248); fill BD[0] (§8.3) with psb_len = ceil((48+n)/128) | BIT15 (OWN)
W8(0x383, R8(0x383) | BIT4)                    // RTK_PCI_TXBD_BCN_WORK |= BIT_PCI_BCNQ_FLAG: kick
poll R32(0x204) bit15 == 1                     // beacon/rsvd page DMA done, 1000 × 10 µs
W16(0x204, rsvd_boundary | BIT15)              // rsvd_boundary is 0 before first mac_init
W8(0x422, tq); W8(0x101, cr1)
```
The BCN ring is not indexed: Linux always reuses BD 0 and frees the previous buffer (pci.c:666-680, 819-820).

### 4.4 Firmware-ready summary
FW is ready when `R16(0x80) == 0xC078`. The same value is used by power_switch to detect a running FW (§2.3).

---------------------------------------------------------------------------------------------------

## 5. eFuse

### 5.1 Physical read (efuse.c:87-123), chip has no `efuse_grant`
```
MASK32(0x34, 0x300, 0)               // REG_LDO_EFUSE_CTRL[9:8] = bank 0 (WiFi)
CLR8(0x37, BIT7)                     // rtw8821c_cfg_ldo25(false): 2.5 V LDO off (read-only access)
c = R32(0x30)                        // REG_EFUSE_CTRL
for addr in 0..511:
    c = (c & ~(0xFF | (0x3FF << 8))) | ((addr & 0x3FF) << 8)
    W32(0x30, c & ~BIT31)            // BIT31 = EF_FLAG, cleared to start a read
    do { udelay(1); c = R32(0x30) } while !(c & BIT31)   // max 1e6 tries
    phy[addr] = c & 0xFF
```
Linux reads with MAC on and FW running (inside `rtw_chip_efuse_enable`); efuse access itself does not need FW.

### 5.2 Logical map (efuse.c:20-85), scan phy[0 .. 512-96 = 416), logical map pre-filled with 0xFF
```
i = 0
while i < 416:
    h1 = phy[i]; h2 = phy[i+1]
    if h1 == 0xFF or ((h1 & 0x1F) == 0x0F and h2 == 0xFF): break
    if (h1 & 0x1F) == 0x0F:  blk = ((h2 & 0xF0) >> 1) | ((h1 >> 5) & 7); wen = h2 & 0xF; i += 2
    else:                    blk = (h1 & 0xF0) >> 4;                     wen = h1 & 0xF; i += 1
    for w in 0..3:
        if wen & (1<<w): continue          // bit set = word NOT present
        log = blk*8 + w*2 ; require i+1 <= 416 and log+1 <= 512 else error
        L[log] = phy[i]; L[log+1] = phy[i+1]; i += 2
```
### 5.3 Logical map layout (`struct rtw8821c_efuse` + `rtw8821ce_efuse`, rtw8821c.h:32-104)
| offset | field | used for |
|---|---|---|
| 0x00 | rtl_id (le16) | — |
| 0x10..0x39 | txpwr_idx_table[0] (path A) | TX power (§7.6) |
| 0x3A..0x63 | txpwr_idx_table[1] (path B) | **2 G part used as path A when rfe_option ∈ {2,4}** (rtw8821c.c:90-91) |
| 0x64..0xB7 | paths C, D | unused |
| 0xB8 | channel_plan | 0xFF→0x7F |
| 0xB9 | xtal_k | crystal cap (0xFF→0) |
| 0xBA | thermal_meter | pwr tracking (skip) |
| 0xBB | iqk_lck | — |
| 0xBC | pa_type | ext PA flags |
| 0xBD/0xBE | lna_type_2g[0..1] | |
| 0xBF/0xC0 | lna_type_5g[0..1] | |
| 0xC1 | rf_board_option | `btcoex = (v & 0xE0) == 0x20`; regd = v & 7 |
| 0xC2 | rf_feature_option | |
| 0xC3 | rf_bt_setting | `share_ant = v & BIT0` |
| 0xC4/0xC5 | eeprom version / customer id | |
| 0xC6 | tx_bb_swing_setting_2g | §7.2 (0xFF→0) |
| 0xC7 | tx_bb_swing_setting_5g | §7.2 (0xFF→0) |
| 0xC8 | tx_pwr_calibrate_rate | |
| 0xC9 | rf_antenna_option | |
| 0xCA | rfe_option | `rfe = v & 0x1F`; `pkg_type = (v & BIT5) ? 1 : 0` |
| 0xCB/0xCC | country_code[2] | ASCII; picks regulatory domain |
| 0xD0..0xD5 | **MAC address** (8821CE) | invalid → Linux uses random |
| 0xD6.. | PCI vendor/device/subsys ids, caps… | — |
`hal.rfe_btg = rfe ∈ {2,4,7,0xA,0xC,0xF}` (rtw8821c.c:76-85). Default fix-ups after parse: main.c:1982-2014.

**Per-path TX power block (42 B, main.h:423-516), path A at 0x10:**
2 G (18 B): `cck_base[6]` @0x10..0x15, `bw40_base[5]` @0x16..0x1A, `ht_1s_diff` @0x1B (low nibble = **ofdm**,
high nibble = **bw20**, both signed 4-bit), ht_2s/3s/4s diffs @0x1C..0x21 (2 B each: [bw20 lo, bw40 hi], [cck lo, ofdm hi]).
5 G (24 B): `bw40_base[14]` @0x22..0x2F, `ht_1s_diff` @0x30 (low = ofdm, high = bw20), ht_2s/3s/4s @0x31..0x33
(low bw20, high bw40), `ofdm_diff` @0x34..0x35, `vht_1s_diff` @0x36 (low bw160, high bw80), vht 2s/3s/4s @0x37..0x39.
Signed nibble: values 8..15 mean −8..−1.

### 5.4 HW feature dump (optional, main.c:1912-1950)
Before FW DL write `W8(0x1A0, 0xFD)`; after FW ready `R8(0x1A0)` must be 0x19, then 13 bytes from 0x1A2;
`W8(0x1A0,0)`. Byte-dword1: bw cap [18:16], nss [20:19], ant_num [23:21]. Not needed for the minimal driver.

---------------------------------------------------------------------------------------------------

## 6. PHY init — `rtw8821c_phy_set_param` (rtw8821c.c:158-205)

### 6.1 RF register access for 8821C
- Read (`rtw_phy_read_rf`, phy.c:894-915): `R32(0x2800 + (addr & 0xFF)*4) & mask` (mask ⊆ 0xFFFFF, shifted down).
- Write (`rtw_phy_write_rf_reg_sipi`, phy.c:966-1004): if mask != 0xFFFFF, read-modify-write with the read above;
  then `W32(0xC90, ((addr & 0xFF) << 20 | (data & 0xFFFFF)) & 0x0FFFFFFF)`; `udelay(13)`.

### 6.2 Exact order
```
v = R8(0x02)
v |= BIT6;            W8(0x02, v)       // BIT_FEN_PCIEA
v |= BIT0|BIT1;       W8(0x02, v)       // BB_RSTB|BB_GLB_RST
v &= ~(BIT0|BIT1);    W8(0x02, v)
v |= BIT0|BIT1;       W8(0x02, v)       // toggle BB reset
W8(0x1F, 0x07); udelay(10)              // REG_RF_CTRL = RF_EN|RF_RSTB|RF_SDM_RSTB
W8(0xEF, 0x07); udelay(10)              // REG_WLRF1+3
CLR32(0x808, BIT28|BIT29)               // REG_RXPSEL &= ~RX_PSEL_RST
load tables (§6.3)
xc = crystal_cap & 0x3F
MASK32(0x24, 0x7E000000, xc)            // REG_AFE_XTAL_CTRL[30:25]
MASK32(0x28, 0x0000007E, xc)            // REG_AFE_PLL_CTRL[6:1]
MASK32(0xA2C, BIT18|BIT22, 0)           // REG_CCK0_FAREPORT
SET32(0x808, BIT28|BIT29)
ch_param[0] = R32(0xA24); ch_param[1] = R32(0xA28); ch_param[2] = R32(0xAAC)   // CCK TX filter defaults
rtw_phy_init: igi0 = R32(0xC50) & 0x7F (no writes for 8821C; adaptivity/cfo ops are NULL, phy.c:200-224)
cck_pd_default = R8(0xAAA) & 0x1F
pwrtrack_init: swing = R32(0xC1C) >> 21; default_ofdm_index = index in rtw8821c_txscale_tbl (else 24)  // SW state
bf init (bf.c:344-378 + rtw8821c.c:151-156):
    t = R32(0x14C0); t |= BIT16; t = (t & ~0xF000) | (0xA << 12); t &= ~BIT7; t &= ~0x3F; W32(0x14C0, t)
    W8(0x167C, 0x70)                    // TXMU ack policy 3<<4 | BIT6
    W16(0x1680, 0)
    SET32(0x42C, BIT30)                 // REG_TXBF_CTRL |= USE_NDPA_PARAMETER
    W8(0x45F, 0x10)                     // NDPA rate
    MASK32(0x6DC, 0x3F000000, 4)        // CSI rate = 6M
    W32(0x1C94, 0xAFFFAFFF)
```
(The bf part is harmless and cheap; it may be skipped [inferred].)

### 6.3 Tables (`rtw_phy_load_tables`, phy.c:1762-1781), order:
1. `rtw8821c_mac` (rtw8821c_table.c:9-149): 138 pairs, **`W8(addr, data)`** (phy.c:1695-1699).
2. `rtw8821c_bb` (1738-3418): 1680 pairs, `W32` except pseudo-addresses 0xFE=msleep 50, 0xFD=5 ms, 0xFC=1 ms,
   0xFB=50 µs, 0xFA=5 µs, 0xF9=1 µs (phy.c:1709-1726) — none present in this table.
3. `rtw8821c_agc` (153-1341): 1200 pairs, `W32` (phy.c:1702-1706) — only 0x81C and 0xC50.
4. **only rfe_option 2 or 4:** `rtw8821c_agc_btg_type2` (1345-1734): 390 pairs, `W32` (rfe_def `.agc_btg_tbl`,
   rtw8821c.c:1627-1628, phy.c:1771-1772).
5. (no `rfk_init_tbl` for 8821C)
6. `rtw8821c_rf_a` (3442-6009): 2712 pairs, RF path A write with mask 0xFFFFF + `udelay(1)`; pseudo-address
   **0xFFE = msleep(50)** (8 occurrences ⇒ ≥400 ms), 0xFE = 100 µs (phy.c:1729-1741).
Power-by-rate (`rtw8821c_bb_pg_type0`, 15 entries) and power-limit (`rtw8821c_txpwr_lmt_type0`, 1488 rows) tables are
parsed into SW arrays at probe (`rtw_chip_board_info_setup`, main.c:2024-2041), never written to HW (§7.6).

### 6.4 Table condition format and evaluation (phy.c:1040-1130, main.h:1802-1832)
Each table is a flat `u32` array of (addr, data) pairs. If `addr` bit31 (`pos`) or bit30 (`neg`) is set, the pair is
control. Condition word layout: `[31] pos, [30] neg, [29:28] branch (0 IF, 1 ELIF, 2 ELSE, 3 ENDIF), [27:24] cut,
[23:20] rsvd, [19:16] plat, [15:12] pkg, [11:8] intf (1 PCIe, 2 USB, 4 SDIO), [7:0] rfe`.
An IF/ELIF is two pairs: `(cond, 0)` then `(0x40000000, 0)` (the "neg" pair triggers evaluation).
Driver condition: `cut = cut_version ? cut_version : 15; pkg = pkg_type ? pkg_type : 15; intf = 1; rfe = rfe_option`.
```
matched = true; skipped = false; pos = 0
for (a, d) in pairs:
    if a & 0x8000_0000:
        br = (a >> 28) & 3
        if br == 3: matched = true; skipped = false          // ENDIF
        elif br == 2: matched = !skipped                       // ELSE
        else: pos = a                                          // IF / ELIF: remember
    elif a & 0x4000_0000:                                      // evaluate remembered cond
        if !skipped:
            if check(pos): matched = true;  skipped = true
            else:          matched = false
        else: matched = false
    elif matched: apply(a, d)
check(c) = (c.cut==0 || c.cut==drv.cut) && (c.pkg==0 || c.pkg==drv.pkg)
        && (c.intf==0 || c.intf==drv.intf) && c.rfe == drv.rfe     // rfe always compared
```
Conditions actually present (enumerated by script): AGC & AGC-BTG: `IF pkg1&rfe4 / ELIF pkg1&rfe5 / ELSE`;
BB: `IF cut C & SDIO / ELSE` and `IF pkg1&rfe5 / ELSE`; RF-A (49 blocks): `IF pkg1&rfe5 / ELIF SDIO&rfe0 /
ELIF pkg1&rfe4 / ELSE`. MAC table has none. ⇒ for PCIe with supported RFE there are only two variants:
**(rfe==4 && pkg_type==1)** and **everything else** (ELSE branches), plus the extra BTG AGC table for rfe 2/4.

### 6.5 Calibration
- `rtw8821c_phy_calibration` = `rtw8821c_do_iqk` only (rtw8821c.c:813-844); no LCK, no DPK. **IQK runs in firmware**:
  H2C packet IQK (§12.2: sub-cmd 0x0E, len 9, word2 bit0 clear=0, bit1 segment_iqk = associated?1:0), then poll RF A
  reg 0x08 (RF_DTXLOK, full mask) until it reads **0xABCDE** (300 × 20 ms max), then write RF 0x08 = 0.
  Diagnostics: `R32(0x1BF0)` bit16 = reload, bits[7:0] = fail mask.
- Linux triggers it via `need_rfk` after a non-scan `rtw_set_channel` (main.c:886-891), executed in
  `mgd_prepare_tx` before auth (main.c:894-902, mac80211.c:696-707). Minimal driver: run once after setting the
  operating channel (needs general/phydm info H2C sent first, §12); skip during channel hopping.
- Power tracking (thermal, rtw8821c.c:1109-1189, every 2 s in the watchdog) — skip for v1.

### 6.6 Dynamic mechanisms (watchdog, main.c:208-290 → phy.c:763-779): DIG (IGI at 0xC50[6:0], min 0x1C),
CCK PD (0xA08[21:16], 0xAA8[20:16], rtw8821c.c:1222-1248), RA, power tracking. **All skippable** for v1; initial
IGI comes from the BB/AGC table.

### 6.7 BT coexistence minimum (antenna to WLAN)
The 8821CE shares RF with BT. Linux path depends on `efuse.btcoex`; for a WLAN-owns-antenna driver mirror the
**wifi-only path** (`wifi_only = true`, coex.c:2706-2710). Exact effect for 8821C:

LTE-coex indirect register access (util.c:24-50, chip addrs rtw8821c.c:1636-1640):
```
read(off):  poll R32(0x1700) bit29 == 1 (1000×10 µs); W32(0x1700, 0x800F0000 | off); v = R32(0x1708)
write(off,v): poll ready; W32(0x1704, v); W32(0x1700, 0xC00F0000 | off)
```
Register 0x38 (`LTE_COEX_CTRL`): GNT_BT in [15:14] and [11:10], GNT_WL in [13:12] and [9:8];
value 0 = HW PTA, 1 = SW low, 3 = SW high (coex.c:951-961, coex.h:114-116).

`rtw_coex_power_on_setting` (coex.c:2725-2749):
```
SET8(0x02, BIT0|BIT1)                       // enable BB so 0x948 is writable
(read scoreboard R16(0xAA) — monitor only)
set_ant_path(POWERON): owner = BT unless BT disabled (0x73 bit2 clear/set); then BBSW/TO_BT ant switch
    — on first start this ant-switch call is a no-op because cur_switch_status==0 (rtw8821c.c:884-885)
coex_table(case 1)                          // see below (share_ant only)
W8(0xFF1A, 0x00)                            // "red x issue"
gnt_debug (rtw8821c.c:974-982): CLR32(0x64,BIT20); CLR32(0x64,BIT24); CLR32(0x40,BIT19);
    CLR32(0x64,BIT15); CLR32(0x70,BIT18); CLR32(0x70,BIT27)
```
`__rtw_coex_init_hw_config(wifi_only=true)` (coex.c:2670-2723):
```
R8(0xF1) (kt version, read only)
H2C 0x69 (BT_WIFI_CONTROL): byte1 = 0x0C (WL_LEAKAP opcode), byte2 = 0x01 (DIS_5MS), bytes3..6 = 0   (coex.c:148-164)
SET8(0x550, BIT3)
rtw8821c_coex_cfg_init (rtw8821c.c:847-872):
    SET8(0x550, BIT3); MASK8(0x790, 0x3F, 5); W8(0x778, 1);
    SET32(0x40, BIT5); SET32(0x40, BIT9);          // BT PTA enable / PTA pins
    SET8(0x4C6, BIT4); CLR8(0x4C6, BIT5);          // PTA WL TX enable, no EDCCA
    SET16(0x762, BIT12);                           // GNT_BT polarity
    MASK8(0x6CF, BIT3, 1)                          // beacon queue hi-pri
WL priority mask (coex.c:355-364): SET bit3 and bit4 of 0x6CC; SET bit3 of 0x6CF
set_ant_path(WONLY) (coex.c:1316-1330):
    LTE 0x38: [15:14]=1, [11:10]=1  (GNT_BT SW low);  [13:12]=3, [9:8]=3 (GNT_WL SW high)
        net effect: v = (v & ~0xFF00) | 0x7700  (Linux does 4 separate RMW cycles)
    SET8(0x73, BIT2)                               // REG_SYS_SDIO_CTRL+3: LTE_MUX_CTRL_PATH = WL owner
    if ant_switch_exist (rfe != 5,6,13,14):  rtw8821c_coex_cfg_ant_switch(BBSW, TO_WLG) (rtw8821c.c:874-969):
        (wlg_at_btg for rfe 2,4,7,10,12,15 → pos becomes polarity?TO_WLA:TO_WLG_BT)
        CLR32(0x4C, BIT23); SET32(0x4C, BIT24)     // DPDT_SEL_EN off, DPDT_WL_SEL on
        MASK8(0xCB4, 0xFF, 0x77)                   // RFE ctrl 8/9 = DPDT
        MASK32(0xCB4, 0xF0000000, regval)          // regval = 2 for rfe 0, 2 and 4 (computed)
        SET8(0x67, BIT5); SET8(0x67, BIT4)
scoreboard: W16(0xAA, 0x8003)                      // BIT1 default | ACTIVE b0 | ONOFF b1 | BT_INT_EN b15
coex_table(case 1): only if share_ant (0xC3 bit0): W32(0x6C0, 0x55555555); W32(0x6C4, 0x55555555);
    W32(0x6C8, 0xF0FFFFFF)   (coex.c:1043-1091; for !share_ant the index underflows and nothing is written)
coex_tdma(case 0): only if share_ant: H2C 0x60 with 5 zero parameters (TDMA off) (coex.c:1193-1257)
H2C 0x61 QUERY_BT_INFO byte1 bit0 = 1 (if BT enabled)
```
Every `set_ant_path` first calls `rtw_coex_check_rfk` (coex.c:411-447): on BTG boards (rfe 2/4) it waits, in 10 ms
steps up to 600 ms, while scoreboard `R16(0xAA)` bit5 (BT RFK) or `R8(0x49C)` bit0 (WL RFK) is set.
`coex->stop_dm = true` in wifi-only mode, so later band-switch notifications do nothing (coex.c:2866-2871).
[inferred] For rfe 0 boards the Linux *coex* 5 GHz phase uses BBSW + **TO_WLA (regval 1)** (coex.c:1355-1370);
the wifi-only path never switches it. If 5 GHz RX is dead on such a board, re-run the ant-switch with TO_WLA on 5 GHz
and TO_WLG on 2.4 GHz. Power-off: `W16(0xAA, 0x8000)`.

---------------------------------------------------------------------------------------------------

## 7. Channel switch (20 MHz)

Linux: `rtw_set_channel` (main.c:850-892) → `rtw_update_channel` (main.c:718-799: current_channel, band,
bw=20, primary idx `RTW_SC_DONT_CARE`=0) → `rtw8821c_set_channel(ch, bw=0, 0)` (rtw8821c.c:576-584) → coex notify
(no-op in wifi-only) → `rtw_phy_set_tx_power_level(ch)` → mark `need_rfk`.
`rtw8821c_set_channel` order: **bb → bb_swing → mac → rf → rxdfir**.

### 7.1 set_channel_bb (rtw8821c.c:443-552)
**2.4 GHz (ch ≤ 14):**
```
MASK32(0x808, BIT28, 1)        // CCK block on
MASK32(0x454, BIT7, 0)
MASK32(0xA80, BIT18, 0)
MASK32(0x814, 0xFC00, 15)
MASK32(0xC1C, 0xF00, 0)
MASK32(0x860, 0x1FFE0000, 0x96A)
if country == CN (SRRC): cck_tx_filter_srrc (rtw8821c.c:384-441), else:
  ch 14: W32(0xA24, 0x0000B81C); MASK32(0xA28, 0xFFFF, 0); W32(0xAAC, 0x00003667)
  else : W32(0xA24, ch_param[0]); MASK32(0xA28, 0xFFFF, ch_param[1] & 0xFFFF); W32(0xAAC, ch_param[2])
```
**5 GHz (ch > 35):**
```
MASK32(0xA80, BIT18, 1); MASK32(0x454, BIT7, 1); MASK32(0x808, BIT28, 0); MASK32(0x814, 0xFC00, 15)
MASK32(0xC1C, 0xF00, 36..64 → 1, 100..144 → 2, ≥149 → 3)
MASK32(0x860, 0x1FFE0000, 36..48 → 0x494, 52..64 → 0x453, 100..116 → 0x452, 118..177 → 0x412)
```
**BW20 (both bands):** `W32(0x8AC, (R32(0x8AC) & 0xFFCFFC00) | 0x10010000)`; `MASK32(0x8C4, BIT30, 1)`.

### 7.2 set_channel_bb_swing (rtw8821c.c:554-574)
`s = (ch ≤ 14 ? efuse[0xC6] : efuse[0xC7])` (0xFF→0, then >9 → 0); `MASK32(0xC1C, 0xFFE00000,
{0x200, 0x16A, 0x101, 0x0B6}[s/3])`; then SW-only `pwrtrack_init`.

### 7.3 rtw_set_channel_mac (mac.c:12-59), 11ac branch
```
W8(0x483, 0)                                  // REG_DATA_SC: txsc20 = txsc40 = 0
W32(0x668, R32(0x668) & ~(BIT7|BIT8))         // REG_WMAC_TRXPTCL_CTL RFMOD = 20 MHz
W32(0x24, R32(0x24) & ~(BIT20|BIT21))         // REG_AFE_CTRL1 MAC_CLK_SEL = 80 MHz (0)
W8(0x55C, 80); W8(0x638, 80)                  // REG_USTIME_TSF / REG_USTIME_EDCA
v = R8(0x454) & ~BIT7; if 5 GHz (36..177): v |= BIT7; W8(0x454, v)   // REG_CCK_CHECK
```
### 7.4 set_channel_rf (rtw8821c.c:310-359; masks rtw8821c.h:297-307)
```
r = RF_A.read(0x18, 0xFFFFF)
r &= ~(BIT16|BIT9|BIT8 | 0xFF | BIT18|BIT17 | BIT11|BIT10)
r |= (ch ≤ 14 ? 0 : BIT16|BIT8) | (ch & 0xFF) | BIT11|BIT10         // band, channel, BW20
if 100 ≤ ch ≤ 140: r |= BIT17  elif ch > 140: r |= BIT18             // RFSI
if ch ≤ 14:
    switch_rf_set(rfe_btg ? BTG : WLG)
    RF_A.write(0xDF, BIT6, 1)                   // RF_LUTDBG
    RF_A.write(0x64, 0xF, 0xF)
else:
    switch_rf_set(WLA)
    RF_A.write(0xDF, BIT6, 0)
RF_A.write(0x18, 0xFFFFF, r)
RF_A.write(0xB8, BIT19, 0); RF_A.write(0xB8, BIT19, 1)              // RF_XTALX2 toggle
```
`switch_rf_set` (rtw8821c.c:276-308):
```
SET32(0x1080, BIT16)                            // REG_DMEM_CTRL |= WL_RST
SET32(0x0000, BIT26)                            // REG_SYS_CTRL |= FEN_EN (= 0x03 bit2, CPU enable)
v = R32(0xCB8)                                  // REG_RFECTL
BTG: v |= BIT16; v &= ~(BIT18|BIT20|BIT22|BIT21|BIT23); MASK32(0xA84, 0xFF0000, 0x0E); MASK32(0xA80, 0xFFFF, 0xFC84)
WLG: v |= BIT20|BIT22|BIT21; v &= ~(BIT16|BIT18|BIT23); MASK32(0xA84, 0xFF0000, 0x12); MASK32(0xA80, 0xFFFF, 0x7532)
WLA: v |= BIT20|BIT22|BIT23; v &= ~(BIT16|BIT18|BIT21)
W32(0xCB8, v)
```
### 7.5 set_channel_rxdfir, BW20 (rtw8821c.c:361-382)
`MASK32(0x948, 0x30000000, 2); MASK32(0x94C, 0x30000000, 2); MASK32(0xC20, BIT31, 1); MASK32(0x8F0, BIT31, 0)`.

### 7.6 TX power
**Registers (rtw8821c.c:723-759):** path A per-rate 6-bit indices, 4 rates per dword at
`0x1D00 + (desc_rate & 0xFC)`, byte `desc_rate & 3`. Sections written (HT_2S/VHT_2S skipped):
| dword | bytes 0..3 |
|---|---|
| 0x1D00 | 1M, 2M, 5.5M, 11M (desc 0x00-0x03) |
| 0x1D04 | 6M, 9M, 12M, 18M |
| 0x1D08 | 24M, 36M, 48M, 54M |
| 0x1D0C | MCS0..MCS3 |
| 0x1D10 | MCS4..MCS7 |
| 0x1D2C | VHT1SS MCS0..3 |
| 0x1D30 | VHT1SS MCS4..7 |
| 0x1D34 | VHT1SS MCS8, MCS9 (bytes 0,1; bytes 2,3 = 0) |
`0xC94[6:1]` (REG_TXAGCIDX) is the power-tracking offset (leave 0). [inferred] ≈0.5 dB per index step.

**Index computation (phy.c:2095-2158), 20 MHz, path A:**
`group` (phy.c:1784-1872): 2.4 G: ch1-2→0, 3-5→1, 6-8→2, 9-11→3, 12-13→4, 14→5 for CCK / 4 for OFDM+;
5 G: 36-42→0, 44-50→1, 52-58→2, 60-64→3, 100-106→4, 108-114→5, 116-122→6, 124-130→7, 132-138→8, 140-144→9,
149-155→10, 157-161→11, 165-171→12, 173-177→13.
```
base:  2G CCK  = cck_base[g]
       2G OFDM = bw40_base[g] + ht_1s_diff.ofdm
       2G HT/VHT-1SS BW20 = bw40_base[g] + ht_1s_diff.bw20
       5G OFDM = bw40_base5[g] + ht_1s_diff5.ofdm
       5G HT/VHT-1SS BW20 = bw40_base5[g] + ht_1s_diff5.bw20
offset = min(by_rate[rate] − by_rate_base[rs],  limit[regd][rs][ch] − by_rate_base[rs],  sar = 63)
idx    = clamp(base + offset, 0, 0x3F)        // Linux clamps only the top; clamp the bottom too
```
By-rate table (`rtw8821c_bb_pg_type0`, rtw8821c_table.c:3422-3438; BCD bytes, byte i ↔ i-th rate of the
register's group, phy.c:1144-1446) resolved to offsets relative to the section base rate
(`rtw_phy_tx_power_by_rate_config`, phy.c:2224-2271; base = last rate, VHT base = MCS7):
| rate | 2 G value / offset | 5 G value / offset |
|---|---|---|
| 1, 2, 5.5, 11M (base 11M) | 38→+6, 36→+4, 34→+2, 32→0 | n/a |
| 6, 9, 12, 18M | 36 → +8 | 34 → +8 |
| 24, 36, 48, 54M (base 54M) | 34 +6, 32 +4, 30 +2, 28 0 | 32 +6, 30 +4, 28 +2, 26 0 |
| MCS0-2, MCS3 | 36 +10, 34 +8 | 34 +10, 32 +8 |
| MCS4..7 (base MCS7) | 32 +6, 30 +4, 28 +2, 26 0 | 30 +6, 28 +4, 26 +2, 24 0 |
| VHT MCS0-7 | same as HT | same as HT |
| VHT MCS8, MCS9 | 24 −2, 22 −4 | 22 −2, 20 −4 |
by_rate_base[rs]: 2 G CCK 32, OFDM 28, HT 26, VHT 26; 5 G OFDM 26, HT 24, VHT 24.
Limits (`rtw8821c_txpwr_lmt_type0`, 1488 rows `{regd, band, bw, rs, ch, limit}`, rtw8821c_table.c:6013-7502;
parse phy.c:1529-1692): regd 0 FCC,1 MKK,2 ETSI,3 IC,4 KCC,5 ACMA,6 CHILE,7 UKRAINE,8 MEXICO,9 CN,10 QATAR,11 UK,
12 WW (WW = per-entry minimum over all regds, phy.c:1549-1557). Missing (63) entries: init to 63; regds without
entries copy an alternative (regd.c `rtw_regd_alt`) or WW; 5 G HT↔VHT cross-filled (phy.c:1563-1640).
Example FCC 2G CCK ch1..13: 30,32,…,32,24,16 ⇒ ch1 CCK offset = min(+6.., 30−32) = −2.
`regd = rtw_regd_get()` = country→`txpwr_regd_2g/5g` (regd.c:494-503); Linux default before a country is set is WW
(regd.c:360).

**Safe first step (no table parsing):** `idx = clamp(base + min(by_rate_off[rate], WW_off[rs][ch]), 0, 0x3F)` with
the world-wide (minimum over all 12 regds) 20 MHz limit offsets `limit − by_rate_base[rs]`, computed offline from
the table by the same rules as Linux (WW = per-entry min, 5 G HT↔VHT cross-fill):
```
2G ch1..13   CCK : -4 -4 -4 -4 -4 -4 -4 -4 -4 -4 -4  -8 -16
             OFDM: +2 +2 +2 +2 +2 +2 +2 +2 +2 -2 -6 -10 -26
             HT  :  0 +4 +4 +4 +4 +4 +4 +4 +4 -2 -6 -10 -26   (VHT 1SS in 2G uses rs VHT; no 2G VHT limits → 63)
5G ch 36 40 44 48 52 56 60 64 100 104 108 112 116 120 124 128 132 136 140 144 149 153 157 161 165
   OFDM +1 +1 +1 +1 -10 +1 +1 +1 +1  +1  +1  +1  +1  +1  +1  +1  +1  +1  +1  +4 -12 -12 -12 -12 -12
   HT/VHT +3 +3 +3 +2 -17 +3 +3 +3 +3 +3  +3  +3  +3  +3  +3  +3  +3  +3  +3  +3 -10 -10 -10 -10 -10
```
Per-regd minima over all channels (limit − base): FCC/IC/MEX/CHILE 2G CCK −16, OFDM −12, HT −14 (ch12/13);
ETSI/QAT 5G band 4 OFDM −12; KCC 5G HT −17 (ch52); CN 2G OFDM/HT −26 (ch13). A single per-regd or WW table like
the one above is enough for `set-country`. If the efuse power bytes are 0xFF (blank) do **not** use them (Linux would
clamp to 0x3F = max power); use a conservative constant base (e.g. 0x20) [inferred].

---------------------------------------------------------------------------------------------------

## 8. PCI rings

### 8.1 Queues, registers (pci.h:44-79), sizes (pci.h:10-15, 239-256)
| queue | enum | DESA (base) | NUM (16-bit) | IDX (host wp [11:0], HW rp [27:16]) | Linux len | qsel | HISR done bit |
|---|---|---|---|---|---|---|---|
| BK | 0 | 0x330 | 0x38A | 0x3AC | 128 | TID | HISR0 b5 |
| BE | 1 | 0x328 | 0x388 | 0x3A8 | 256 | TID (0 for non-QoS) | HISR0 b4 |
| VI | 2 | 0x320 | 0x386 | 0x3A4 | 128 | TID | HISR0 b3 |
| VO | 3 | 0x318 | 0x384 | 0x3A0 | 128 | TID | HISR0 b2 |
| BCN | 4 | 0x308 | — | — (kick 0x383 bit4) | 1 | 0x10 | — |
| MGMT | 5 | 0x310 | 0x380 | 0x3B0 | 128 | 0x12 | HISR0 b6 |
| HI0 | 6 | 0x340 | 0x38C | 0x3B8 | 128 | 0x11 (+MORE_DATA) | HISR0 b7 |
| H2C | 7 | 0x1320 | 0x1328 | 0x132C | 128 | 0x13 | HISR3 b16 |
| RX MPDU | — | 0x338 | 0x382 | 0x3B4 (host rp [11:0], HW wp [27:16]) | 512 | — | HISR0 b0 |
Needed: **MGMT, BE, H2C, BCN (FW download), RX**. [inferred] Program all rings anyway (tiny 2-entry rings for
BK/VI/VO/HI0) since Linux always does; ring length ≤ 4095 (`TRX_BD_IDX_MASK`, pci.c:186).
Frame → queue (tx.c:726-746): beacon → BCN; mgmt or ctrl → MGMT; addr1 multicast/broadcast data → HI0;
else AC (BE).

### 8.2 Ring programming — `rtw_pci_setup` = reset_buf_desc + dma_reset (pci.c:394-472, 518-534)
Called before power-on, and again after FW download (and on stop):
```
W8(0x303, R8(0x303) | 0xF7)   // RTK_PCI_CTRL[31:24]; [inferred] max RX/TX DMA burst (halmac naming)
W32(0x308, bcn_ring_dma)
W16(0x1328, h2c_len); W32(0x1320, h2c_dma)
W16(0x38A, bk_len);   W32(0x330, bk_dma)
W16(0x388, be_len);   W32(0x328, be_dma)
W16(0x384, vo_len);   W32(0x318, vo_dma)
W16(0x386, vi_len);   W32(0x320, vi_dma)
W16(0x380, mgmt_len); W32(0x310, mgmt_dma)
W16(0x38C, hi0_len);  W32(0x340, hi0_dma)
W16(0x382, rx_len);   W32(0x338, rx_dma)
(all SW wp = rp = 0)
W32(0x39C, 0xFFFFFFFF)                  // RTK_PCI_TXBD_RWPTR_CLR: clear all TX/RX indices
SET32(0x1330, BIT16|BIT8)               // clear H2C host/HW index
SET32(0x300, BIT20|BIT15)               // RST_TRXDMA_INTF | RX_TAG_EN ; SW rx_tag = 0
```
### 8.3 TX buffer descriptor (ring entry = 16 B = 2 × `rtw_pci_tx_buffer_desc`, pci.h:165-169)
```
seg[n]: le16 buf_size @+0, le16 psb_len @+2, le32 dma @+4
seg0.buf_size = 48 (TX desc size); seg0.psb_len = ceil((48 + frame_len)/128)  [| BIT15 OWN for BCN only]
seg0.dma      = phys(TX desc)       // desc and frame contiguous in one buffer
seg1.buf_size = frame_len; seg1.psb_len = 0; seg1.dma = phys(TX desc) + 48
```
(pci.c:834-844). Zero the 16 bytes first.
### 8.4 Kick and completion
Kick: advance SW wp (mod len), `W16(IDX_q, wp)` (pci.c:771-786). Full test: `avail = rp > wp ? rp−wp−1 : len−wp+rp−1`
(pci.h:154-160) must be > 0. Completion: `hw_rp = (R32(IDX_q) >> 16) & 0xFFF`; entries from SW rp up to hw_rp are
done and their buffers can be freed (pci.c:952-1011). Linux does **not** derive ACK from completion; ACK comes from
the CCX report (§9.6). H2C ring entries are just freed.

### 8.5 RX ring (pci.c:208-312, 1022-1116)
Entry = 8 B `rtw_pci_rx_buffer_desc` (pci.h:193-197): `le16 buf_size, le16 total_pkt_size, le32 dma`.
Init/refill each entry: zero it, `buf_size = 11478` (`RTK_PCI_RX_BUF_SIZE` = 11454 + 24), `dma = phys(buffer)`.
Linux: 512 entries. HW writes `total_pkt_size` = running RX tag (0..8191) used only as a DMA sanity check
(pci.c:682-701).
Polling loop:
```
hw_wp = (R32(0x3B4) >> 16) & 0xFFF
n = hw_wp >= sw ? hw_wp − sw : len − (sw − hw_wp)
repeat n times: process buffer[sw] (§10); re-arm BD[sw] (zero, buf_size, dma); sw = (sw+1) % len
W16(0x3B4, sw)                          // return buffers (host read index)
```
If buffers are made smaller than Linux's, lower `REG_RX_PKT_LIMIT 0x60C` (units of 512 B) so that
`limit*512 + 24 + 32 + 8 ≤ buf_size` [inferred]. DMA is cache-coherent on x86; elsewhere sync before reading.

---------------------------------------------------------------------------------------------------

## 9. TX descriptor (48 B, `struct rtw_tx_desc` tx.h:12-59, filled by `rtw_tx_fill_tx_desc` tx.c:35-89)

### 9.1 Fields used by Linux (dword, bits)
| dw | bits | field | notes |
|---|---|---|---|
| 0 | 15:0 | TXPKTSIZE | frame length (no desc) |
| 0 | 23:16 | OFFSET | 48 (desc size) — 0 for H2C packets |
| 0 | 24 | BMC | addr1 group address |
| 0 | 26 | LS | 1 |
| 0 | 31 | DISQSELSEQ | 1 for mgmt with HW seq |
| 1 | 7:0 | MACID | 0 (first vif) |
| 1 | 12:8 | QSEL | §8.1 |
| 1 | 20:16 | RATE_ID | §9.3 |
| 1 | 23:22 | SEC_TYPE | 0 (no HW crypto) |
| 1 | 28:24 | PKT_OFFSET | 0 on PCIe |
| 1 | 29 | MORE_DATA | 1 only for qsel HIGH |
| 2 | 12 | AGG_EN | 0 |
| 2 | 19 | **SPE_RPT** | request CCX TX report |
| 2 | 22:20 | AMPDU_DENSITY | 0 |
| 2 | 23 | BT_NULL | 0 |
| 3 | 7:6 | HW_SSN_SEL | 0 |
| 3 | 8 | **USE_RATE** | use DATARATE (no firmware RA) |
| 3 | 10 | **DISDATAFB** | disable rate fallback |
| 3 | 12 | USE_RTS | 0 |
| 3 | 15 | NAVUSEHDR | PS-Poll only |
| 3 | 21:17 | MAX_AGG_NUM | 0 |
| 4 | 6:0 | **DATARATE** | DESC_RATE (main.h:241-334): 0x00 1M,0x01 2M,0x02 5.5M,0x03 11M,0x04 6M…0x0B 54M, 0x0C MCS0…0x13 MCS7, 0x2C VHT1SS MCS0…0x35 MCS9 |
| 4 | 28:24 | RTSRATE | only if RTS (24M) |
| 5 | 4 | DATA_SHORT | short GI (HT/VHT) / short preamble (CCK) [vendor comment]; Linux 0 |
| 5 | 6:5 | DATA_BW | 0 = 20 MHz |
| 5 | 7 | DATA_LDPC | 0 |
| 5 | 9:8 | DATA_STBC | 0 |
| 6 | 11:0 | **SW_DEFINE** | report cookie: bits[7:2] = 6-bit counter, [1:0] = 0 (tx.c:162-173) |
| 7 | 15:0 | TXDESC_CHECKSUM | **not used on PCIe** (USB/SDIO only) |
| 8 | 15 | EN_HWSEQ | 1 for mgmt (HW sequence number) |
| 9 | 23:12 | SW_SEQ | header seq for data frames |
| 9 | 7 / 6:0 | TIM_EN / TIM_OFFSET | beacons only |
dw10, dw11 = 0.

### 9.2 Additional fields **[vendor]** (halmac_tx_desc_nic.h; used by the 8821C vendor driver for mgmt frames,
`SET_TX_DESC_RTY_LMT_EN_8821C(…,1)` + `RTS_DATA_RTY_LMT 12 or 6`, vendor rtl8821c_ops.c:3209-3214)
| dw | bits | field |
|---|---|---|
| 4 | 17 | RTY_LMT_EN — use per-packet limit |
| 4 | 23:18 | RTS_DATA_RTY_LMT — retry limit (6 bits) |
| 4 | 12:8 | DATA_RTY_LOWEST_RATE |
| 4 | 7 | TRY_RATE |
| 5 | 31 | POLLUTED |
| 8 | 13:8 | DATA_RC (retry counter, HW-maintained) |
Global alternative (also vendor): `REG_RETRY_LIMIT 0x42A` (16-bit, SRL [13:8], LRL [5:0]); rtw88 writes it only in
coex limited-TX mode (`0x0808` = 8/8, coex.c:76). Not set by the MAC table; hardware default unknown.

### 9.3 Recommended fill for the target API
**Management frame, fixed 1 Mbit/s (2.4 G) / 6 Mbit/s (5 G)** (Linux `rtw_tx_mgmt_pkt_info_update` +
`rtw_tx_pkt_info_update_rate`, tx.c:268-324; ring MGMT):
```
dw0 = len | 48<<16 | bmc<<24 | 1<<26 | 1<<31
dw1 = 0 /*macid*/ | 0x12<<8 | (2G ? 8 /*RATEID_B_20M*/ : 7 /*RATEID_G*/)<<16
dw2 = (report ? 1<<19 : 0)
dw3 = 1<<8 | 1<<10                         // USE_RATE | DISDATAFB
dw4 = (2G ? 0x00 : 0x04) [| 1<<17 | (limit & 0x3F)<<18  — vendor]
dw6 = (cookie & 0x3F) << 2
dw8 = 1<<15                                // HW seq (Linux); or 0 and dw9 = seq<<12 for SW seq
```
**Data frame, fixed OFDM/HT rate** (ring BE, Linux data path tx.c:326-395 with `fix_rate` set → use_rate+disdatafb):
```
dw0 = len | 48<<16 | bmc<<24 | 1<<26
dw1 = 0 | 0 /*qsel TID0*/<<8 | rate_id<<16
      rate_id: OFDM 2G 6 (BG) or 7 (G); HT 2G 3 (BGN_20M_1SS); HT 5G 5 (GN_N1SS); VHT 5G 10; VHT 2G 11 (main.h:221-239)
dw2 = report<<19
dw3 = 1<<8 | 1<<10
dw4 = desc_rate [| vendor retry fields]
dw6 = cookie<<2
dw9 = (hdr_seq & 0xFFF) << 12              // EN_HWSEQ = 0
```
Firmware rate adaptation is **not required**: with USE_RATE=1 the MAC uses DATARATE; RA is only used when USE_RATE=0
(Linux data path without fix_rate) and then needs H2C RA_INFO 0x40 per macid (fw.c:725-750). No H2C needed for
fixed-rate TX [inferred from code paths].

**Reserved-page/FW chunk on BCN ring** (tx.c:441-515): qsel 0x10, OFFSET 48, LS, DISQSELSEQ, EN_HWSEQ, USE_RATE,
DISDATAFB, rate 6M/rate_id G (current_band_type is 0 at that time), BMC = whatever bytes 4..9 of the chunk look like
(irrelevant). **H2C packet on H2C ring** (tx.c:518-541): only TXPKTSIZE = 32 and QSEL = 0x13; everything else 0.

### 9.4 Sequence numbers
Linux: mgmt/null frames → EN_HWSEQ=1, DISQSELSEQ=1, HW_SSN_SEL=0 (hardware assigns); data → SW_SEQ = header seq.
Since the station above sets sequence numbers, SW_SEQ + EN_HWSEQ=0 for all frames is the consistent choice
[inferred: equals Linux data-frame handling].

### 9.5 Requesting a report
Set SPE_RPT and SW_DEFINE = `(n & 0x3F) << 2` (64 outstanding cookies). Linux matches the report by
`cookie & 0xFC` and times out after 500 ms (tx.h:10, tx.c:175-206).

### 9.6 TX report (C2H, arrives in RX ring — §10.3)
Two formats handled by Linux (fw.c:93-115, 304-307; tx.c:224-253; fw.h:357-360). `payload` = C2H bytes from offset 2.
- **V1 — C2H id 0xFF (C2H_HALMAC), payload[0] = 0x0F (C2H_CCX_RPT)**. Expected for 8821C (11ac firmware)
  [inferred: V0 was added for 11n-CPU chips]. Linux: cookie = `payload[8] & 0xFC`, status = `payload[9] & 0xC0`,
  acked = status == 0. Full layout **[vendor]** (halmac_fw_offload_c2h_nic.h:207-265, offsets relative to C2H start
  = payload+2): `payload[1]` len; `payload[3][4:0]` QSEL; `payload[4][6:0]` MACID; `payload[5][6:0]` initial rate;
  `payload[6..7]` queue time (le16); `payload[8]` SW_DEFINE byte0; `payload[9][3:0]` RTS retry count, `[5]` BMC,
  `[7:6]` TX_STATE (0 = OK); **`payload[10][5:0]` DATA retry count**; `payload[11][6:0]` final data rate.
- **V0 — C2H id 0x03 (C2H_CCX_TX_RPT)**: cookie = `payload[6] & 0xFC`, status = `payload[0] & 0xC0`
  (bit6 retry-over, bit7 lifetime-over). [inferred from rtlwifi `GET_TX_REPORT_RETRY_V1`] retry count = `payload[2] & 0x3F`.
Implement both; log raw bytes until confirmed. `attempts_used = data_retry_count + 1` [inferred; verify against a
deliberately absent receiver with a known limit].

---------------------------------------------------------------------------------------------------

## 10. RX descriptor and PHY status

### 10.1 RX descriptor, 24 B (rx.h:17-44; confirmed by vendor halmac_rx_desc_nic.h)
| dw | bits | field |
|---|---|---|
| 0 | 13:0 | PKT_LEN (frame incl. FCS, or C2H length) |
| 0 | 14 | CRC32 error |
| 0 | 15 | ICV error |
| 0 | 19:16 | DRV_INFO_SIZE (units of 8 B) |
| 0 | 22:20 | ENC_TYPE (0 none) |
| 0 | 25:24 | SHIFT (bytes) |
| 0 | 26 | PHYST (PHY status present in drv info) |
| 0 | 27 | SWDEC (HW did not decrypt) |
| 1 | 6:0 | MACID |
| 2 | 28 | **C2H** (firmware message, not a frame) |
| 2 | 30:29 | PPDU_CNT |
| 3 | 6:0 | RX_RATE (DESC_RATE) |
| 4 | 5:4 | BW |
| 5 | 31:0 | TSF low |
(rtw8821c.c:682-721.) Frame start = buf + 24 + DRV_INFO_SIZE*8 + SHIFT; length PKT_LEN; **last 4 bytes are FCS**
(RCR `APP_FCS`, Linux sets `RX_INCLUDES_FCS`, main.c:2223) — strip them. Drop if CRC32 set.

### 10.2 PHY status (drv info, 32 B since `REG_RX_DRVINFO_SZ = 4`), at buf + 24 + SHIFT (rtw8821c.c:613-680; macros rtw8821c.h:177-211)
`page = byte0 & 0xF`.
- **page 0 (CCK):** `vga = dw3[12:8]`, `lna = (dw3[23] << 3) | dw3[15:13]`;
  `dBm = lna_gain[lna] − 2*vga`, table = rfe_option==0 ? {22,8,-6,-22,-31,-40,-46,-52} :
  {10,6,2,-2,-6,-10,-14,-17,-20,-24,-28,-31,-34,-37,-40,-44}; out-of-range lna → −120.
- **page 1 (OFDM/HT/VHT):** `dBm = dw0[15:8] (PWDB_A) − 110`, floored at −120. RXSC dw1[11:8]/[15:12], RF mode dw3[29:28].
- other pages: ignore.

### 10.3 C2H packets in the RX ring
If `dw2 bit28` is set, the buffer holds a firmware message at offset `24 + DRV_INFO_SIZE*8 + SHIFT`
(pci.c:1070-1086): `byte0 = id`, `byte1 = seq`, `payload = bytes 2..`, `payload_len = PKT_LEN − 2` (fw.c:335-372).
Relevant ids (fw.h:51-71): 0x03 CCX_TX_RPT, 0xFF HALMAC (payload[0] sub-id: 0x0F CCX_RPT, 0x03 scan status, 0x22
chan switch), 0x09 BT info, 0x0C RA report, 0x27 WLAN info, 0x0B BT MP info, 0x32 WLAN RFON, 0x36 beacon filter,
0x37 adaptivity, 0x38 scan result. Everything except CCX may be dropped.

---------------------------------------------------------------------------------------------------

## 11. Receive filtering

### 11.1 RCR 0x608 (bits reg.h:449-481)
Linux final value after start: **0xF410400E** = APP_FCS b31 | APP_MIC b30 | APP_ICV b29 | APP_PHYSTS b28 | VHT_DACK b26 |
PKTCTL_DLEN b20 | HTC_LOC_CTRL b14 | AB b3 | AM b2 | APM b1 (main.c:2141-2143, 1864, 1498).
`configure_filter` (mac80211.c:276-322): FIF_ALLMULTI → AM; FIF_FCSFAIL → ACRC32 b8; FIF_OTHER_BSS → AAP b0;
FIF_BCN_PRBRESP_PROMISC set → clear CBSSID_BCN b7 and CBSSID_DATA b6, cleared → set CBSSID_BCN.
Mapping to `set-filter(all_beacons, bssid, own_mac)`:
- all_beacons = true (scan): RCR = 0xF410400E (CBSSID_BCN = 0) — beacons/probe responses from every BSS pass
  (RXFLTMAP0 0x6A0 = all mgmt subtypes), unicast frames pass only if addr1 == own MAC (APM), group frames via AB/AM.
- all_beacons = false: write BSSID, RCR |= BIT7 (CBSSID_BCN) → 0xF410408E.
  Linux never sets CBSSID_DATA (b6) in station mode; setting it would also drop group-addressed data from other BSSs [inferred].
- ACRC32 off: CRC-failed frames are not delivered.

### 11.2 Address registers, port 0 (`rtw_vif_port[0]`, mac80211.c:108-115)
| what | register | write |
|---|---|---|
| own MAC | **0x610..0x615** (REG_MACID) | 6 × W8 (main.c:904-910) |
| BSSID | 0x618..0x61D | 6 × W8 |
| network type | 0x100 [17:16] | MASK32(0x100, 0x30000, t): 0 no link, 1 adhoc, 2 station linked, 3 AP |
| AID | 0x6A8 [10:0] | only for PS |
| beacon ctrl | 0x550 (8-bit) | 0x08 (EN_BCN_FUNCTION) for station |
Linux: add_interface writes MAC, net type 0, bcn_ctrl 0x08 (mac80211.c:199-217); on association net type 2 + AID
(main.c:562-577, mac80211.c:387-413); BSSID on BSS_CHANGED_BSSID. Recommend net type 2 once the station is
associated [inferred: TSF sync from own BSS].

### 11.3 Other filter registers
`0x6A0` = 0x0FFFFFFF (mgmt all; ctrl subtypes 12..15 masked), `0x6A4` = 0xFFFF (data all) from mac_init.
Multicast hash `REG_MAR 0x620..0x627` = 0xFF… from MAC table.

### 11.4 Security engine
Linux enables it at start (sec.c:124-145: `SET16(0x100, BIT9)`, `SET16(0x680, 0xCF)`). With software crypto and no
CAM entries, **skip it**; protected frames are delivered undecrypted (`SWDEC` set) [inferred].

---------------------------------------------------------------------------------------------------

## 12. H2C commands

### 12.1 Mailbox commands (8 bytes) — `rtw_fw_send_h2c_command` (fw.c:436-489)
```
box = h2c_box (0..3, reset to 0 after every FW download)
poll R8(0x1CC) (REG_HMETFR) bit[box] == 0, every 100 µs, ≤ 3 ms
W32(0x1F0 + 4*box, le32(bytes 4..7))     // REG_HMEBOXn_EX first
W32(0x1D0 + 4*box, le32(bytes 0..3))     // REG_HMEBOXn — triggers FW
box = (box + 1) & 3
```
byte0 = command id (fw.h:544-576). Used in the minimal flow:
| id | name | bytes | when |
|---|---|---|---|
| 0xD1 | RECOVER_BT_DEV | b1 bit0 = 1 | after each FW download (PCIe 8821C, mac.c:991-993) |
| 0x69 | BT_WIFI_CONTROL | b1 = opcode 0x0C, b2 = 0x01, b3..b6 = 0 | coex init (§6.7) |
| 0x60 | COEX_TDMA_TYPE | b1..b5 = 0 | coex init, share_ant only |
| 0x61 | QUERY_BT_INFO | b1 bit0 = 1 | coex init (optional) |
| 0x01 | MEDIA_STATUS_RPT | b1 bit0 = connected, b2 = macid | on association (optional) |
Not needed: 0x40 RA_INFO (only for firmware rate adaptation), 0x00 RSVD_PAGE / 0x20 SET_PWR_MODE (power save),
0x2C DEFAULT_PORT (LPS timer), 0x42 RSSI, 0x56/0x57 beacon filter, 0x66 WL channel info (coex).

### 12.2 H2C packets (32 bytes) through the H2C TX ring — `rtw_fw_send_h2c_packet` (fw.c:496-507, pci.c:894-913)
```
dw0 = 0x01 (category) | 0xFF<<8 (cmd id) | sub_id<<16
dw1 = total_len (= 8 + payload) | seq<<16          // seq: u16 counter, reset to 0 after FW download
dw2.. = payload
```
Buffer = 48-byte TX desc (dw0 = 32, dw1 = 0x13<<8, rest 0) + 32-byte packet; BD as §8.3; kick `W16(0x132C, wp)`.
| sub_id | name | total_len | payload (dw2) | fw.c |
|---|---|---|---|---|
| 0x0D | GENERAL_INFO | 12 | [23:16] = fw_txbuf_addr − rsvd_boundary = 508 − 460 = **48** | 509-528 |
| 0x11 | PHYDM_INFO | 16 | [7:0] rfe_option, [15:8] rf type (FW_RF_1T1R = **4**), [23:16] cut_version, [27:24] = 1, [31:28] = 1 | 530-557 |
| 0x0E | IQK | 9 | bit0 clear = 0, bit1 segment = 0/1 | 559-570 |
Send GENERAL_INFO and PHYDM_INFO once after mac_init+phy_set_param (main.c:1400-1402), before IQK.

---------------------------------------------------------------------------------------------------

## 13. Consolidated minimal init sequence (recommended)
```
 1  PCI: Mem+BusMaster; map BAR2; DEVCTL2 |= COMP_TMOUT_DIS; (MDIO 0x09←0x6380, §1.1)
 2  v = R32(0xF0): cut, rf type (must be 1T1R)
 3  allocate rings + buffers < 4 GiB; program rings (§8.2)
 4  MAC power on (§2.3)  (handles "already on" by cycling)
 5  FW download (§4.2, which ends by re-programming the rings §8.2), then H2C mailbox 0xD1
 6  efuse read/parse (§5), first time only; refuse rfe ∉ {0,2,4,6} like Linux (or accept at own risk)
 7  mac_init (§3: 3.1→3.6)
 8  phy_set_param incl. tables (§6.2, 6.3)
 9  H2C GENERAL_INFO, PHYDM_INFO (§12.2)
10  coex power-on + wifi-only init (§6.7)
11  W32(0x608, 0xF410400E)   (skip sec engine)
12  port 0: MAC 0x610, MASK32(0x100,0x30000,0), W8(0x550, 0x08)
13  set_channel (§7) + TX power (§7.6); IQK (§6.5) for the operating channel
14  poll RX ring / TX rings
Stop: HIMR=0, re-program rings, W16(0xAA,0x8000), power off (§2.3)
```
Linux does steps 4–6 once at probe, powers off, then repeats 3–5 and continues; a single pass is equivalent as far
as the code shows [inferred]. Writing `W8(0x1A0,0xFD)` before FW DL is only needed for the HW-feature dump.

---------------------------------------------------------------------------------------------------

## 14. What can be skipped for a first working version
- IRQ/MSI (poll), ASPM/CLKREQ/link PS, DBI, Intel-bridge workaround.
- LPS/IPS/deep PS, RPWM/CPWM handshakes (keep only the RPWM toggle in power_switch), WoWLAN, PNO, scan offload,
  beacon filter offload, reserved pages other than FW download, DTIM/TIM.
- Security engine and CAM; beamforming (rtw_bf_* writes optional); MU-MIMO.
- Firmware rate adaptation (H2C RA_INFO), RA reports, RRSR updates, AMPDU/AMSDU/BA.
- DIG, CCK PD, false-alarm statistics, power tracking/thermal, EDCCA/adaptivity, CFO tracking, SAR.
- Coex state machine (use wifi-only path), BT info parsing, TDMA tables (non-shared antenna writes nothing anyway).
- 40/80 MHz branches, 2SS rate sections, HW-feature dump, efuse write, HI0/BK/VI/VO data use (rings still programmed).
- IQK can be postponed for RX-only bring-up; needed for good TX EVM at higher rates.

## 15. Risks
1. **TX report format/retry count**: V0 vs V1 not proven for the current 8821C firmware; retry-count byte positions come
   from vendor headers/rtlwifi, not rtw88.
2. **Per-packet retry limit** (dw4 RTY_LMT_EN/RTS_DATA_RTY_LMT) is vendor-only; rtw88 never sets it.
3. **RFE support**: Linux supports rfe 0/2/4/6 only; tables for other values are untested; blank efuse (0xFF) ⇒ rfe 31.
4. **Antenna/coex**: wifi-only path forces GNT_WL; on combo cards BT may still be active; rfe-0 5 GHz DPDT position
   (§6.7) uncertain.
5. **TX power legality**: without the limit table some channels exceed FCC/ETSI limits (ch12/13 FCC, 5 G band 4
   ETSI); blank efuse power bytes ⇒ max power. DFS channels (52-144) must be RX-only (no radar detection).
6. **32-bit DMA only**; non-coherent platforms need cache maintenance.
7. FW-ready poll is only ~10 ms in Linux; give it more (e.g. 100 ms). DDMA/LLT/BCN-valid polls are 10 ms.
8. IQK may block up to 6 s (300 × 20 ms); RF table load sleeps ≥ 400 ms.
9. Polling latency: RX ring overflow if not drained (HW drops); TX report timeouts if C2H not drained.
10. PCIe completion-timeout disable and ASPM issues reported for 8821CE (Linux quirks pci.c:1496-1503, 1767-1769).
11. Firmware blob licence/storage must follow the project's proprietary-file rule (fetched, not committed).
12. Crystal cap 0 when efuse blank (Linux behaviour) ⇒ frequency offset.

## 16. Size estimate
- Tables: MAC 138 + BB 1680 + AGC 1200 + AGC-BTG 390 + RF-A 2712 = **6120 u32 pairs** (~6.1 k lines one-pair-per-line,
  ~49 KB), by-rate 15 entries, power limits 1488 rows (~1.5 k lines; or a pre-computed WW 20 MHz table of
  ~(14 ch × 3 rs) + (25 ch × 3 rs) bytes), power sequences 30 entries, coex constants < 20 lines.
  Pre-evaluating conditions offline for one (rfe, pkg) pair would cut RF/AGC tables by ~15 %.
- Logic (Rust, no_std): PCI/rings/TX/RX/C2H ~700 lines; power + MAC init + FW download ~500; efuse ~120;
  PHY (table evaluator, RF access, phy_set_param, set_channel, TX power) ~600; coex minimum + H2C ~200;
  glue/API ~300 ⇒ **~2,400 lines of logic** against **~7,700 lines of tables** (if transcribed 1:1).
