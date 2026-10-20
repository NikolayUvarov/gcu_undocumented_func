// The MIND Link multi-device (800-LNK-0005): two of these boards, joined by two wires and ground, each a USB device
// to its own machine. The serial and data ports pass bytes to the other board's same port; lines on the control port
// drive the other board's keyboard, mouse and tablet; the LED shows the link, or the boot of the machine that sends
// the indicator's lines. docs/link/multi-device.md.
#include "bridge.h"
#include "control.h"
#include "led.h"
#include "line.h"
#include "usb_ids.h"
#include "bsp/board_api.h"
#include "hardware/clocks.h"
#include "pico/status_led.h"
#include "pico/stdlib.h"
#include "pico/unique_id.h"
#include "tusb.h"
#include <stdio.h>
#include <string.h>

enum { HELLO_MS = 200, PEER_LOST_MS = 1000, CREDIT_MS = 5, CREDIT_BYTES = 512, KEY_QUEUE = 512, MOUSE_QUEUE = 256 };
static const uint8_t PORT_OF[2] = {CDC_SERIAL, CDC_DATA};

static bridge_tx_stream tx[2];                 // our ports' bytes to the other board
static bridge_rx_stream rx[2];                 // its bytes to our ports
static uint8_t inbox[2][BRIDGE_WINDOW];        // received, not yet taken by our host
static uint32_t inbox_head[2], inbox_len[2];
static uint32_t credited[2], credit_ms, refused;
static bridge_rx parser;
static led_state led;
static control_state ctl;
static uint32_t peer_ms;
static char peer_name[40] = "none";

// HID reports waiting for their interface: keyboard (modifiers and six keys), mouse, and the last tablet position.
static uint8_t keys[KEY_QUEUE][7];
static uint32_t keys_head, keys_len;
static struct { uint8_t buttons; int8_t x, y, wheel; } moves[MOUSE_QUEUE];
static uint32_t moves_head, moves_len;
static uint8_t tablet[5];
static bool tablet_pending;

static uint32_t now_ms(void) { return to_ms_since_boot(get_absolute_time()); }

static bool send(uint8_t type, uint8_t channel, uint32_t offset, const uint8_t *payload, size_t n) {
    uint8_t wire[BRIDGE_WIRE];
    if (line_room() < BRIDGE_WIRE) return false;
    line_write(wire, bridge_frame(type, channel, offset, payload, n, wire));
    return true;
}

static void reply(const char *text) {
    tud_cdc_n_write_str(CDC_CONTROL, text);
    tud_cdc_n_write_flush(CDC_CONTROL);
}

static void key(uint8_t modifiers, const uint8_t usages[6]) {
    if (keys_len == KEY_QUEUE) return;
    uint8_t *k = keys[(keys_head + keys_len++) % KEY_QUEUE];
    k[0] = modifiers; memcpy(k + 1, usages, 6);
}

static void move(uint8_t buttons, int dx, int dy, int wheel) {
    // A move larger than a report's ±127 goes as several.
    do {
        int sx = dx > 127 ? 127 : dx < -127 ? -127 : dx, sy = dy > 127 ? 127 : dy < -127 ? -127 : dy;
        if (moves_len == MOUSE_QUEUE) return;
        uint32_t at = (moves_head + moves_len++) % MOUSE_QUEUE;
        moves[at].buttons = buttons; moves[at].x = (int8_t)sx; moves[at].y = (int8_t)sy; moves[at].wheel = (int8_t)wheel;
        dx -= sx; dy -= sy; wheel = 0;
    } while (dx || dy);
}

// A message from the other board's control port, played on our HID interfaces.
static void play(const uint8_t *m, size_t n) {
    static const uint8_t none[6] = {0};
    if (n == 8 && m[0] == MSG_KEYS) { key(m[1], m + 2); return; }
    if (n >= 2 && m[0] == MSG_TEXT) {
        for (size_t i = 1; i < n; i++) {
            uint8_t usage, press[6] = {0}; bool shift;
            if (!ascii_usage((char)m[i], &usage, &shift)) continue;
            press[0] = usage;
            key(shift ? 0x02 : 0, press);
            key(0, none);
        }
        return;
    }
    if (n == 7 && m[0] == MSG_MOUSE) { move(m[1], (int16_t)(m[2] | m[3] << 8), (int16_t)(m[4] | m[5] << 8), (int8_t)m[6]); return; }
    if (n == 6 && m[0] == MSG_ABS) { memcpy(tablet, m + 1, 5); tablet_pending = true; }
}

static void hid_task(void) {
    if (!tud_mounted()) { keys_len = moves_len = 0; tablet_pending = false; return; }
    if (keys_len && tud_hid_n_ready(HID_KEYBOARD)) {
        uint8_t *k = keys[keys_head];
        if (tud_hid_n_keyboard_report(HID_KEYBOARD, 0, k[0], k + 1)) { keys_head = (keys_head + 1) % KEY_QUEUE; keys_len--; }
    }
    if (moves_len && tud_hid_n_ready(HID_MOUSE)) {
        uint32_t at = moves_head;
        if (tud_hid_n_mouse_report(HID_MOUSE, 0, moves[at].buttons, moves[at].x, moves[at].y, moves[at].wheel, 0)) {
            moves_head = (moves_head + 1) % MOUSE_QUEUE; moves_len--;
        }
    }
    if (tablet_pending && tud_hid_n_ready(HID_TABLET) && tud_hid_n_report(HID_TABLET, 0, tablet, sizeof tablet)) tablet_pending = false;
}

static void control_line(const char *line) {
    control_action a;
    control_parse(&ctl, line, &a);
    char text[200];
    bool peer = peer_ms && now_ms() - peer_ms < PEER_LOST_MS;
    switch (a.kind) {
    case CTL_INDICATOR: led_line(&led, line, now_ms()); return;  // the indicator's lines get no answer
    case CTL_HELLO: {
        char serial[2 * PICO_UNIQUE_BOARD_ID_SIZE_BYTES + 1];
        pico_get_unique_board_id_string(serial, sizeof serial);
        snprintf(text, sizeof text, "MIND Link 1 %s %s peer %s\n", LINK_BOARD_NAME, serial, peer ? peer_name : "none");
        reply(text);
        return;
    }
    case CTL_STATUS:
        snprintf(text, sizeof text, "peer %s sent %lu %lu received %lu %lu lost %lu %lu damaged %lu overruns %lu refused %lu\n",
                 peer ? "up" : "down", (unsigned long)tx[0].sent, (unsigned long)tx[1].sent, (unsigned long)rx[0].next,
                 (unsigned long)rx[1].next, (unsigned long)rx[0].lost, (unsigned long)rx[1].lost, (unsigned long)parser.dropped,
                 (unsigned long)line_overruns(), (unsigned long)refused);
        reply(text);
        return;
    case CTL_ERROR: snprintf(text, sizeof text, "err %s\n", a.error); reply(text); return;
    case CTL_PEER:
        if (!peer) { reply("err no other board\n"); return; }
        for (int i = 0; i < a.count; i++) if (!send(FT_DATA, CH_CONTROL, 0, a.msg[i], a.len[i])) { reply("err the line is full\n"); return; }
        reply("ok\n");
        return;
    }
}

static void control_task(void) {
    static char line[256];
    static size_t len;
    static bool too_long;
    while (tud_cdc_n_available(CDC_CONTROL)) {
        char c = (char)tud_cdc_n_read_char(CDC_CONTROL);
        if (c == '\r') continue;
        if (c != '\n') { if (len < sizeof line - 1) line[len++] = c; else too_long = true; continue; }
        line[len] = 0;
        if (too_long) reply("err line too long\n"); else if (len) control_line(line);
        len = 0; too_long = false;
    }
}

static void received(const bridge_msg *m, uint32_t now) {
    peer_ms = now;
    if (m->type == FT_HELLO) {
        size_t n = m->len < sizeof peer_name - 1 ? m->len : sizeof peer_name - 1;
        memcpy(peer_name, m->payload, n); peer_name[n] = 0;
        return;
    }
    if (m->type == FT_CREDIT && m->len == 8) {
        for (int c = 0; c < 2; c++) bridge_credit(&tx[c], (uint32_t)m->payload[4 * c] | (uint32_t)m->payload[4 * c + 1] << 8 | (uint32_t)m->payload[4 * c + 2] << 16 | (uint32_t)m->payload[4 * c + 3] << 24);
        return;
    }
    if (m->type != FT_DATA) return;
    if (m->channel == CH_CONTROL) { play(m->payload, m->len); return; }
    if (m->channel > CH_DATA) return;
    int c = m->channel;
    size_t skip, keep = bridge_accept(&rx[c], m->offset, m->len, &skip);
    for (size_t i = 0; i < keep; i++) {
        if (inbox_len[c] == BRIDGE_WINDOW) { refused++; continue; }  // beyond its credit: the other board misbehaved
        inbox[c][(inbox_head[c] + inbox_len[c]++) % BRIDGE_WINDOW] = m->payload[skip + i];
    }
    if (keep) led.activity_ms = now;
}

static void bridge_task(uint32_t now) {
    static uint32_t hello_ms;
    uint8_t bytes[256];
    size_t n;
    // From the other board.
    while ((n = line_read(bytes, sizeof bytes)) > 0) {
        bridge_msg m;
        for (size_t i = 0; i < n; i++) if (bridge_rx_push(&parser, bytes[i], &m)) received(&m, now);
    }
    // To our host's ports; dropped while nobody has a port open, as a serial line's bytes are.
    for (int c = 0; c < 2; c++) {
        uint8_t port = PORT_OF[c];
        while (inbox_len[c]) {
            uint32_t take = inbox_len[c], start = inbox_head[c] % BRIDGE_WINDOW;
            if (take > BRIDGE_WINDOW - start) take = BRIDGE_WINDOW - start;
            if (tud_cdc_n_connected(port)) {
                uint32_t room = tud_cdc_n_write_available(port);
                if (take > room) take = room;
                if (!take) break;
                tud_cdc_n_write(port, inbox[c] + start, take);
            }
            inbox_head[c] += take; inbox_len[c] -= take;
        }
        tud_cdc_n_write_flush(port);
    }
    // From our host's ports, within the other board's credit.
    for (int c = 0; c < 2; c++) {
        uint8_t port = PORT_OF[c];
        uint32_t room = bridge_room(&tx[c]);
        uint32_t avail = tud_cdc_n_available(port);
        n = avail < room ? avail : room;
        if (n > BRIDGE_PAYLOAD) n = BRIDGE_PAYLOAD;
        if (n == 0 || line_room() < BRIDGE_WIRE) continue;
        uint8_t data[BRIDGE_PAYLOAD];
        n = tud_cdc_n_read(port, data, (uint32_t)n);
        if (n && send(FT_DATA, (uint8_t)c, tx[c].sent, data, n)) { tx[c].sent += (uint32_t)n; led.activity_ms = now; }
    }
    // Credits: how far our host has taken each stream (bytes lost on the line count as taken).
    uint32_t consumed[2] = {rx[0].next - inbox_len[0], rx[1].next - inbox_len[1]};
    bool moved = consumed[0] - credited[0] >= CREDIT_BYTES || consumed[1] - credited[1] >= CREDIT_BYTES;
    bool due = (consumed[0] != credited[0] || consumed[1] != credited[1]) && now - credit_ms >= CREDIT_MS;
    if (moved || due || now - credit_ms >= HELLO_MS) {
        uint8_t p[8];
        for (int c = 0; c < 2; c++) for (int i = 0; i < 4; i++) p[4 * c + i] = (uint8_t)(consumed[c] >> (8 * i));
        if (send(FT_CREDIT, 0, 0, p, sizeof p)) { credited[0] = consumed[0]; credited[1] = consumed[1]; credit_ms = now; }
    }
    if (now - hello_ms >= HELLO_MS) {
        char serial[2 * PICO_UNIQUE_BOARD_ID_SIZE_BYTES + 1];
        pico_get_unique_board_id_string(serial, sizeof serial);
        if (send(FT_HELLO, 0, 0, (const uint8_t *)serial, strlen(serial))) hello_ms = now;
    }
}

static void led_task(uint32_t now) {
    static uint32_t last = UINT32_MAX;
    led.host = tud_mounted();
    led.peer = peer_ms && now - peer_ms < PEER_LOST_MS;
    static uint32_t errors;
    uint32_t e = parser.dropped + line_overruns() + rx[0].lost + rx[1].lost + refused;
    if (e != errors) { errors = e; led.error_ms = now ? now : 1; }
    rgb c = led_colour(&led, now);
    uint32_t colour = PICO_COLORED_STATUS_LED_COLOR_FROM_RGB(c.r, c.g, c.b);
    if (colour == last) return;
    last = colour;
    if (colored_status_led_supported()) colored_status_led_set_on_with_color(colour);
    else status_led_set_state(colour != 0);
}

int main(void) {
    // 120 MHz: the line's 8 PIO cycles a bit divide it evenly at 15 Mbit/s.
    set_sys_clock_khz(120000, true);
    board_init();
    status_led_init();
    line_start(LINK_BAUD);
    bridge_rx_init(&parser);
    tud_init(0);
    for (;;) {
        uint32_t now = now_ms();
        tud_task();
        bridge_task(now);
        control_task();
        hid_task();
        led_task(now);
    }
}

// HID GET_REPORT and SET_REPORT (the host's keyboard LEDs): nothing to give, nothing kept.
uint16_t tud_hid_get_report_cb(uint8_t instance, uint8_t id, hid_report_type_t type, uint8_t *buffer, uint16_t len) {
    (void)instance; (void)id; (void)type; (void)buffer; (void)len;
    return 0;
}
void tud_hid_set_report_cb(uint8_t instance, uint8_t id, hid_report_type_t type, const uint8_t *buffer, uint16_t len) {
    (void)instance; (void)id; (void)type; (void)buffer; (void)len;
}
