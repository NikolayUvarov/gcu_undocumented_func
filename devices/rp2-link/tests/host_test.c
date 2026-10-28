// The multi-device firmware's logic on the host (800-LNK-0005): the frames between the boards, their credits, the
// control port's commands and the LED's table. Run by scripts/host_tests.sh.
#include "../src/bridge.h"
#include "../src/control.h"
#include "../src/led.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures;
#define CHECK(cond) do { if (!(cond)) { printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); failures++; } } while (0)

static uint64_t rng = 0x9E3779B97F4A7C15ull;
static uint32_t next(void) { rng ^= rng >> 12; rng ^= rng << 25; rng ^= rng >> 27; return (uint32_t)((rng * 0x2545F4914F6CDD1Dull) >> 32); }

static void crc_and_cobs(void) {
    CHECK(crc16((const uint8_t *)"123456789", 9) == 0x29B1);  // CRC-16/CCITT-FALSE check value
    for (size_t n = 0; n <= 600; n += (n < 260 ? 1 : 37)) {
        uint8_t in[600], enc[700], dec[700];
        for (size_t i = 0; i < n; i++) in[i] = (i % 7 == 0) ? 0 : (uint8_t)next();
        size_t e = cobs_encode(in, n, enc);
        CHECK(memchr(enc, 0, e) == NULL);
        CHECK(e <= n + n / 254 + 1);
        CHECK(cobs_decode(enc, e, dec) == n);
        CHECK(memcmp(in, dec, n) == 0);
    }
    uint8_t bad[] = {5, 1, 2};
    uint8_t out[8];
    CHECK(cobs_decode(bad, sizeof bad, out) == SIZE_MAX);
}

static void frames_round_trip_and_damage(void) {
    uint8_t payload[BRIDGE_PAYLOAD], wire[BRIDGE_WIRE];
    for (int i = 0; i < BRIDGE_PAYLOAD; i++) payload[i] = (uint8_t)(i * 7);
    size_t n = bridge_frame(FT_DATA, CH_DATA, 0xFFFFFF00u, payload, BRIDGE_PAYLOAD, wire);
    CHECK(n <= BRIDGE_WIRE && wire[n - 1] == 0);
    bridge_rx rx; bridge_rx_init(&rx);
    bridge_msg m; int got = 0;
    for (size_t i = 0; i < n; i++) got += bridge_rx_push(&rx, wire[i], &m);
    CHECK(got == 1 && m.type == FT_DATA && m.channel == CH_DATA && m.offset == 0xFFFFFF00u && m.len == BRIDGE_PAYLOAD);
    CHECK(memcmp(m.payload, payload, BRIDGE_PAYLOAD) == 0);
    // Every single changed byte is caught.
    for (size_t at = 0; at + 1 < n; at += 5) {
        uint8_t bad[BRIDGE_WIRE]; memcpy(bad, wire, n);
        bad[at] ^= 0x41; if (bad[at] == 0) continue;
        bridge_rx r; bridge_rx_init(&r); int g = 0;
        for (size_t i = 0; i < n; i++) g += bridge_rx_push(&r, bad[i], &m);
        CHECK(g == 0 && r.dropped == 1);
    }
    // Noise, then an overlong run, then a good frame.
    bridge_rx r; bridge_rx_init(&r); int g = 0;
    for (int i = 0; i < 100000; i++) g += bridge_rx_push(&r, (next() % 50) ? (uint8_t)next() : 0, &m);
    for (int i = 0; i < 3 * BRIDGE_WIRE; i++) bridge_rx_push(&r, 0x55, &m);
    bridge_rx_push(&r, 0, &m);
    g = 0;
    for (size_t i = 0; i < n; i++) g += bridge_rx_push(&r, wire[i], &m);
    CHECK(g == 1 && m.len == BRIDGE_PAYLOAD);
}

static void credits(void) {
    bridge_tx_stream s = {0, 0};
    CHECK(bridge_room(&s) == BRIDGE_WINDOW);
    s.sent = BRIDGE_WINDOW;
    CHECK(bridge_room(&s) == 0);
    bridge_credit(&s, 1000);
    CHECK(s.consumed == 1000 && bridge_room(&s) == 1000);
    bridge_credit(&s, 500);           // an old credit
    CHECK(s.consumed == 1000);
    bridge_credit(&s, BRIDGE_WINDOW + 5);  // past what was sent
    CHECK(s.consumed == 1000);
    // Across the wrap.
    bridge_tx_stream w = {0xFFFFFF00u + 300, 0xFFFFFF00u};
    bridge_credit(&w, 0xFFFFFF00u + 200);
    CHECK(w.consumed == 0xFFFFFF00u + 200 && bridge_room(&w) == BRIDGE_WINDOW - 100);
}

static void accepting(void) {
    bridge_rx_stream s = {0, 0};
    size_t skip;
    CHECK(bridge_accept(&s, 0, 100, &skip) == 100 && skip == 0 && s.next == 100);
    CHECK(bridge_accept(&s, 0, 100, &skip) == 0);                       // a repeat
    CHECK(bridge_accept(&s, 50, 100, &skip) == 50 && skip == 50 && s.next == 150);
    CHECK(bridge_accept(&s, 300, 10, &skip) == 10 && s.lost == 150 && s.next == 310);  // a frame lost
    bridge_rx_stream w = {0xFFFFFFF0u, 0};
    CHECK(bridge_accept(&w, 0xFFFFFFF0u, 32, &skip) == 32 && w.next == 0x10);
}

// A whole channel through a line that loses frames: what arrives is in order, and the window never overflows.
static void lossy_stream(void) {
    bridge_tx_stream tx = {0, 0};
    bridge_rx_stream rxs = {0, 0};
    bridge_rx rx; bridge_rx_init(&rx);
    uint32_t delivered = 0, buffered = 0, lost_frames = 0;
    uint8_t wire[BRIDGE_WIRE], data[BRIDGE_PAYLOAD];
    for (int round = 0; round < 20000; round++) {
        uint32_t room = bridge_room(&tx);
        size_t n = room < BRIDGE_PAYLOAD ? room : BRIDGE_PAYLOAD;
        if (n) {
            for (size_t i = 0; i < n; i++) data[i] = (uint8_t)(tx.sent + i);
            size_t w = bridge_frame(FT_DATA, CH_SERIAL, tx.sent, data, n, wire);
            tx.sent += (uint32_t)n;
            if (next() % 100 < 3) { wire[w / 2] ^= 0x10; if (!wire[w / 2]) wire[w / 2] = 1; lost_frames++; }
            bridge_msg m;
            for (size_t i = 0; i < w; i++) if (bridge_rx_push(&rx, wire[i], &m)) {
                size_t skip, keep = bridge_accept(&rxs, m.offset, m.len, &skip);
                for (size_t k = 0; k < keep; k++) CHECK(m.payload[skip + k] == (uint8_t)(m.offset + skip + k));
                buffered += (uint32_t)keep;
                CHECK(buffered <= BRIDGE_WINDOW);
            }
        }
        // The receiver drains some and sends a credit: what it consumed, lost bytes counted as consumed.
        uint32_t take = next() % 600; if (take > buffered) take = buffered;
        buffered -= take; delivered += take;
        if (next() % 4 == 0) bridge_credit(&tx, rxs.next - buffered);
    }
    CHECK(delivered > 1000000 && rxs.lost > 0 && rx.dropped == lost_frames);
}

static void control_lines(void) {
    control_state st = {0};
    control_action a;
    control_parse(&st, "key ctrl+alt+delete", &a);
    CHECK(a.kind == CTL_PEER && a.count == 2 && a.msg[0][0] == MSG_KEYS && a.msg[0][1] == 0x05 && a.msg[0][2] == 0x4C && a.msg[1][1] == 0 && a.msg[1][2] == 0);
    control_parse(&st, "key F5", &a);
    CHECK(a.kind == CTL_PEER && a.msg[0][2] == 0x3E);
    control_parse(&st, "key f13", &a); CHECK(a.kind == CTL_ERROR);
    control_parse(&st, "key f1x", &a); CHECK(a.kind == CTL_ERROR);
    control_parse(&st, "key a+b+c+d+e+f+g", &a); CHECK(a.kind == CTL_ERROR);
    control_parse(&st, "key gui+r", &a); CHECK(a.kind == CTL_PEER && a.msg[0][1] == 0x08 && a.msg[0][2] == 0x15);
    control_parse(&st, "type Hello, World!", &a);
    CHECK(a.kind == CTL_PEER && a.count == 1 && a.msg[0][0] == MSG_TEXT && a.len[0] == 14 && memcmp(a.msg[0] + 1, "Hello, World!", 13) == 0);
    control_parse(&st, "type \xd0\xbf", &a); CHECK(a.kind == CTL_ERROR);
    control_parse(&st, "move -5 7", &a);
    CHECK(a.kind == CTL_PEER && a.msg[0][0] == MSG_MOUSE && (int16_t)(a.msg[0][2] | a.msg[0][3] << 8) == -5 && a.msg[0][4] == 7);
    control_parse(&st, "move 1", &a); CHECK(a.kind == CTL_ERROR);
    control_parse(&st, "press right", &a); CHECK(st.buttons == BTN_RIGHT && a.msg[0][1] == BTN_RIGHT);
    control_parse(&st, "move 3 3", &a); CHECK(a.msg[0][1] == BTN_RIGHT);
    control_parse(&st, "release right", &a); CHECK(st.buttons == 0 && a.msg[0][1] == 0);
    control_parse(&st, "click", &a); CHECK(a.count == 2 && a.msg[0][1] == BTN_LEFT && a.msg[1][1] == 0);
    control_parse(&st, "abs 16384 32767", &a); CHECK(a.kind == CTL_PEER && a.msg[0][0] == MSG_ABS && (a.msg[0][4] | a.msg[0][5] << 8) == 32767);
    control_parse(&st, "abs 0 40000", &a); CHECK(a.kind == CTL_ERROR);
    control_parse(&st, "hello", &a); CHECK(a.kind == CTL_HELLO);
    control_parse(&st, "@STATE READY", &a); CHECK(a.kind == CTL_INDICATOR);
    control_parse(&st, "frobnicate", &a); CHECK(a.kind == CTL_ERROR);
    // Every printable ASCII character but none has a key.
    for (char ch = ' '; ch < 127; ch++) { uint8_t u; bool sh; CHECK(ascii_usage(ch, &u, &sh)); }
    uint8_t u; bool sh;
    CHECK(ascii_usage('?', &u, &sh) && u == 0x38 && sh);
    CHECK(ascii_usage('\n', &u, &sh) && u == 0x28 && !sh);
}

static void led_table(void) {
    led_state s = {0};
    rgb c = led_colour(&s, 1000); CHECK(c.b && !c.r && !c.g);            // powered, no host: dim blue
    s.host = true; c = led_colour(&s, 1000); CHECK(c.g && c.b && !c.r);   // host, no peer: cyan
    s.peer = true; c = led_colour(&s, 1000); CHECK(c.g && !c.b && !c.r);  // both: green
    s.error_ms = 900; c = led_colour(&s, 1000); CHECK(c.r && !c.g);       // a damaged frame: red blinking
    CHECK(led_line(&s, "@STATE SERVICES 15/30", 2000) && s.stage == BOOT_SERVICES && s.started == 15 && s.all == 30);
    c = led_colour(&s, 2000); CHECK(c.r && c.g && !c.b);                  // yellow
    CHECK(led_line(&s, "@BEAT 2000", 2000));
    CHECK(led_line(&s, "@STATE READY", 2100)); c = led_colour(&s, 2100); CHECK(!c.r && c.g);
    CHECK(led_line(&s, "@HEALTH WARN disk", 2200)); c = led_colour(&s, 2200); CHECK(c.r && c.g && c.r > c.g);
    CHECK(led_line(&s, "@HEALTH FAIL x", 2300)); c = led_colour(&s, 2300); CHECK(c.r && !c.g);
    c = led_colour(&s, 5500); CHECK((c.r && c.b && !c.g) || (!c.r && !c.g && !c.b));  // beats stopped: magenta blinking
    CHECK(!led_line(&s, "hello", 6000));
}

// A board started again: the other one sees its new boot number once.
static void restarts(void) {
    uint32_t known = 0;
    CHECK(bridge_peer_restarted(&known, 0x1234));    // the first hello
    CHECK(!bridge_peer_restarted(&known, 0x1234));
    CHECK(bridge_peer_restarted(&known, 0x9999));    // a new start
    CHECK(!bridge_peer_restarted(&known, 0x9999) && known == 0x9999);
}

int main(void) {
    restarts();
    crc_and_cobs();
    frames_round_trip_and_damage();
    credits();
    accepting();
    lossy_stream();
    control_lines();
    led_table();
    if (failures) { printf("%d FAILED\n", failures); return 1; }
    printf("rp2-link host tests: all passed\n");
    return 0;
}
