// The LED's table (led.h): plain C, built for the boards and for the host tests.
#include "led.h"
#include <stdlib.h>
#include <string.h>

enum { DIM = 12, MID = 40, BEAT_LOST_MS = 3000, FLASH_MS = 120 };

static bool starts(const char *line, const char *word) { size_t n = strlen(word); return strncmp(line, word, n) == 0 && (line[n] == ' ' || line[n] == 0); }

bool led_line(led_state *s, const char *line, uint32_t now_ms) {
    if (starts(line, "@BEAT")) { s->beat_ms = now_ms ? now_ms : 1; return true; }
    if (starts(line, "@HEALTH")) {
        const char *v = line + 7; while (*v == ' ') v++;
        s->health = starts(v, "FAIL") ? HEALTH_FAIL : starts(v, "WARN") ? HEALTH_WARN : HEALTH_OK;
        return true;
    }
    if (!starts(line, "@STATE")) return false;
    const char *v = line + 6; while (*v == ' ') v++;
    boot_stage stage = starts(v, "KERNEL") ? BOOT_KERNEL : starts(v, "SERVICES") ? BOOT_SERVICES : starts(v, "READY") ? BOOT_READY
                     : starts(v, "REBOOT") ? BOOT_REBOOT : starts(v, "OFF") ? BOOT_OFF : BOOT_NONE;
    if (stage == BOOT_NONE) return true;
    if (stage == BOOT_SERVICES) {
        const char *n = v + 8; while (*n == ' ') n++;
        char *slash; long a = strtol(n, &slash, 10);
        long b = *slash == '/' ? strtol(slash + 1, NULL, 10) : 0;
        if (a >= 0 && b > 0 && a <= b && b < 65536) { s->started = (uint16_t)a; s->all = (uint16_t)b; }
    }
    if (stage != s->stage) s->stage_ms = now_ms;
    s->stage = stage;
    return true;
}

static rgb c(uint8_t r, uint8_t g, uint8_t b) { rgb x = {r, g, b}; return x; }

rgb led_colour(const led_state *s, uint32_t now) {
    bool blink = (now / 250) % 2 == 0;
    if (s->stage != BOOT_NONE) {
        // The boot indicator's table (issues-human section 8).
        if (s->stage == BOOT_REBOOT || s->stage == BOOT_OFF) return (now - s->stage_ms) % 1000 < 200 ? c(MID, MID, MID) : c(0, 0, 0);
        if (s->beat_ms && now - s->beat_ms > BEAT_LOST_MS) return blink ? c(MID, 0, MID) : c(0, 0, 0);
        if (s->health == HEALTH_FAIL) return c(MID, 0, 0);
        if (s->health == HEALTH_WARN) return c(MID, MID / 3, 0);
        if (s->stage == BOOT_READY) return c(0, MID, 0);
        uint32_t level = s->stage == BOOT_SERVICES && s->all ? DIM + (uint32_t)(MID - DIM) * s->started / s->all : DIM;
        return c((uint8_t)level, (uint8_t)level, 0);
    }
    if (s->error_ms && now - s->error_ms < 1000) return blink ? c(MID, 0, 0) : c(0, 0, 0);
    if (!s->host) return c(0, 0, DIM);
    if (!s->peer) return c(0, DIM, DIM);
    if (s->activity_ms && now - s->activity_ms < FLASH_MS) return c(0, MID, 0);
    return c(0, DIM, 0);
}
