// The RGB LED's colour (800-LNK-0005): the link's state, or, once the host sends the indicator's lines (`@STATE`,
// `@HEALTH`, `@BEAT`; issues-human section 8), how far that machine's boot has come. One table, kept dim.
#ifndef LED_H
#define LED_H
#include <stdbool.h>
#include <stdint.h>

typedef struct { uint8_t r, g, b; } rgb;

typedef enum { BOOT_NONE, BOOT_KERNEL, BOOT_SERVICES, BOOT_READY, BOOT_REBOOT, BOOT_OFF } boot_stage;
typedef enum { HEALTH_OK, HEALTH_WARN, HEALTH_FAIL } health;

typedef struct {
    bool host;                  // this board's USB is configured
    bool peer;                  // the other board answers
    uint32_t activity_ms;       // last data moved (0: never)
    uint32_t error_ms;          // last damaged frame or lost byte (0: never)
    boot_stage stage;           // from @STATE; BOOT_NONE until one comes
    uint16_t started, all;      // @STATE SERVICES <started>/<all>
    health health;              // from @HEALTH
    uint32_t beat_ms;           // last @BEAT (0: none yet)
    uint32_t stage_ms;          // when the stage last changed
} led_state;

// An indicator line from the host (`@STATE ...`, `@HEALTH ...`, `@BEAT ...`); false if not one.
bool led_line(led_state *s, const char *line, uint32_t now_ms);
rgb led_colour(const led_state *s, uint32_t now_ms);

#endif
