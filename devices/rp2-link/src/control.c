// The control port's commands (control.h): plain C, built for the boards and for the host tests.
#include "control.h"
#include <stdlib.h>
#include <string.h>

typedef struct { const char *name; uint8_t usage, modifier; } key_name;
static const key_name KEYS[] = {
    {"enter", 0x28, 0}, {"return", 0x28, 0}, {"esc", 0x29, 0}, {"escape", 0x29, 0}, {"backspace", 0x2A, 0},
    {"tab", 0x2B, 0}, {"space", 0x2C, 0}, {"capslock", 0x39, 0}, {"printscreen", 0x46, 0}, {"scrolllock", 0x47, 0},
    {"pause", 0x48, 0}, {"insert", 0x49, 0}, {"home", 0x4A, 0}, {"pageup", 0x4B, 0}, {"delete", 0x4C, 0}, {"del", 0x4C, 0},
    {"end", 0x4D, 0}, {"pagedown", 0x4E, 0}, {"right", 0x4F, 0}, {"left", 0x50, 0}, {"down", 0x51, 0}, {"up", 0x52, 0},
    {"menu", 0x65, 0},
    {"ctrl", 0, 0x01}, {"control", 0, 0x01}, {"shift", 0, 0x02}, {"alt", 0, 0x04}, {"gui", 0, 0x08}, {"win", 0, 0x08},
    {"super", 0, 0x08}, {"rctrl", 0, 0x10}, {"rshift", 0, 0x20}, {"ralt", 0, 0x40}, {"altgr", 0, 0x40}, {"rgui", 0, 0x80},
};

// Unshifted and shifted characters on the US layout's keys from 0x1E (1) to 0x38 (/), in usage order; '\0' none.
static const char PLAIN[] = "1234567890\n\x1b\b\t -=[]\\\0;'`,./";
static const char SHIFTED[] = "!@#$%^&*()\0\0\0\0\0_+{}|\0:\"~<>?";

bool ascii_usage(char c, uint8_t *usage, bool *shift) {
    if (c >= 'a' && c <= 'z') { *usage = (uint8_t)(0x04 + c - 'a'); *shift = false; return true; }
    if (c >= 'A' && c <= 'Z') { *usage = (uint8_t)(0x04 + c - 'A'); *shift = true; return true; }
    for (size_t i = 0; i < sizeof PLAIN - 1; i++) {
        if (PLAIN[i] && PLAIN[i] == c) { *usage = (uint8_t)(0x1E + i); *shift = false; return true; }
        if (SHIFTED[i] && SHIFTED[i] == c) { *usage = (uint8_t)(0x1E + i); *shift = true; return true; }
    }
    return false;
}

static bool same(const char *a, size_t n, const char *b) { return strlen(b) == n && strncmp(a, b, n) == 0; }

bool key_usage(const char *name, size_t n, uint8_t *usage, uint8_t *modifier) {
    *usage = 0; *modifier = 0;
    if (n == 0) return false;
    for (size_t i = 0; i < sizeof KEYS / sizeof KEYS[0]; i++)
        if (same(name, n, KEYS[i].name)) { *usage = KEYS[i].usage; *modifier = KEYS[i].modifier; return true; }
    if ((name[0] == 'f' || name[0] == 'F') && n >= 2 && n <= 3 && name[1] != '0') {
        int k = 0;
        for (size_t i = 1; i < n; i++) { if (name[i] < '0' || name[i] > '9') { k = 0; break; } k = k * 10 + name[i] - '0'; }
        if (k >= 1 && k <= 12) { *usage = (uint8_t)(0x3A + k - 1); return true; }
    }
    if (n == 1) { bool shift; if (ascii_usage(name[0] >= 'A' && name[0] <= 'Z' ? (char)(name[0] + 32) : name[0], usage, &shift) && !shift) return true; }
    return false;
}

static void error(control_action *out, const char *why) { out->kind = CTL_ERROR; out->error = why; out->count = 0; }

static void put16(uint8_t *p, int v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }

// The next word of `*s`, its length; `*s` moves past it and the spaces after.
static size_t word(const char **s, const char **start) {
    while (**s == ' ') (*s)++;
    *start = *s;
    while (**s && **s != ' ') (*s)++;
    size_t n = (size_t)(*s - *start);
    while (**s == ' ') (*s)++;
    return n;
}

static bool number(const char **s, long min, long max, long *v) {
    const char *w; size_t n = word(s, &w);
    if (n == 0 || n > 11) return false;
    char buf[12]; memcpy(buf, w, n); buf[n] = 0;
    char *end; long x = strtol(buf, &end, 10);
    if (*end || x < min || x > max) return false;
    *v = x; return true;
}

static uint8_t button(const char *w, size_t n) {
    if (n == 0 || same(w, n, "left")) return BTN_LEFT;
    if (same(w, n, "right")) return BTN_RIGHT;
    if (same(w, n, "middle")) return BTN_MIDDLE;
    return 0;
}

static void mouse(control_action *out, int slot, uint8_t buttons, long dx, long dy, long wheel) {
    uint8_t *m = out->msg[slot];
    m[0] = MSG_MOUSE; m[1] = buttons; put16(m + 2, (int)dx); put16(m + 4, (int)dy); m[6] = (uint8_t)(int8_t)wheel;
    out->len[slot] = 7;
    out->count = (uint8_t)(slot + 1);
}

void control_parse(control_state *st, const char *line, control_action *out) {
    memset(out, 0, sizeof *out);
    out->kind = CTL_PEER;
    const char *s = line, *w;
    size_t n = word(&s, &w);
    if (n == 0) { error(out, "empty line"); return; }
    if (w[0] == '@') { out->kind = CTL_INDICATOR; return; }
    if (same(w, n, "hello")) { out->kind = CTL_HELLO; return; }
    if (same(w, n, "status")) { out->kind = CTL_STATUS; return; }
    if (same(w, n, "type")) {
        size_t len = strlen(s);
        if (len == 0) { error(out, "nothing to type"); return; }
        if (len > sizeof out->msg[0] - 1) { error(out, "too long: type at most 249 characters a line"); return; }
        for (size_t i = 0; i < len; i++) { uint8_t u; bool sh; if (!ascii_usage(s[i], &u, &sh)) { error(out, "a character without a key on the US layout"); return; } }
        out->msg[0][0] = MSG_TEXT; memcpy(out->msg[0] + 1, s, len); out->len[0] = (uint8_t)(len + 1); out->count = 1;
        return;
    }
    if (same(w, n, "key")) {
        // key ctrl+alt+delete: the chord pressed, then everything released.
        const char *chord; size_t cn = word(&s, &chord);
        if (cn == 0 || *s) { error(out, "key takes one chord, as ctrl+alt+delete"); return; }
        uint8_t mods = 0, keys[6] = {0}, nk = 0;
        for (size_t i = 0; i < cn;) {
            size_t j = i; while (j < cn && chord[j] != '+') j++;
            uint8_t usage, mod;
            if (!key_usage(chord + i, j - i, &usage, &mod)) { error(out, "an unknown key name"); return; }
            if (usage) { if (nk == 6) { error(out, "at most six keys at once"); return; } keys[nk++] = usage; }
            mods |= mod;
            i = j + 1;
        }
        uint8_t *p = out->msg[0];
        p[0] = MSG_KEYS; p[1] = mods; memcpy(p + 2, keys, 6); out->len[0] = 8;
        uint8_t *r = out->msg[1];
        r[0] = MSG_KEYS; memset(r + 1, 0, 7); out->len[1] = 8;
        out->count = 2;
        return;
    }
    if (same(w, n, "move")) {
        long dx, dy, wheel = 0;
        if (!number(&s, -32767, 32767, &dx) || !number(&s, -32767, 32767, &dy) || (*s && !number(&s, -127, 127, &wheel)) || *s) { error(out, "move <dx> <dy> [<wheel>]"); return; }
        mouse(out, 0, st->buttons, dx, dy, wheel);
        return;
    }
    if (same(w, n, "wheel")) {
        long v;
        if (!number(&s, -127, 127, &v) || *s) { error(out, "wheel <-127..127>"); return; }
        mouse(out, 0, st->buttons, 0, 0, v);
        return;
    }
    if (same(w, n, "click") || same(w, n, "press") || same(w, n, "release")) {
        const char *b; size_t bn = word(&s, &b);
        uint8_t bit = button(b, bn);
        if (!bit || *s) { error(out, "a button is left, right or middle"); return; }
        if (w[0] == 'c') { mouse(out, 0, st->buttons | bit, 0, 0, 0); mouse(out, 1, st->buttons, 0, 0, 0); return; }
        if (w[0] == 'p') st->buttons |= bit; else st->buttons &= (uint8_t)~bit;
        mouse(out, 0, st->buttons, 0, 0, 0);
        return;
    }
    if (same(w, n, "abs")) {
        long x, y;
        if (!number(&s, 0, 32767, &x) || !number(&s, 0, 32767, &y) || *s) { error(out, "abs <x> <y>, each 0..32767 across the screen"); return; }
        uint8_t *m = out->msg[0];
        m[0] = MSG_ABS; m[1] = st->buttons; put16(m + 2, (int)x); put16(m + 4, (int)y);
        out->len[0] = 6; out->count = 1;
        return;
    }
    error(out, "unknown command");
}
