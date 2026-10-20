// The control port's lines (800-LNK-0005): text commands from this board's host. Input commands become messages to
// the other board, which plays them on its own USB keyboard, mouse and tablet; the rest is answered here.
#ifndef CONTROL_H
#define CONTROL_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Messages on the control channel to the other board.
enum { MSG_KEYS = 'K', MSG_TEXT = 'T', MSG_MOUSE = 'M', MSG_ABS = 'A' };
// Mouse buttons.
enum { BTN_LEFT = 1, BTN_RIGHT = 2, BTN_MIDDLE = 4 };

typedef struct {
    uint8_t buttons;  // held by `press`, sent with each move
} control_state;

// What a line asks for: up to two messages to the other board (a press and its release), or a local question.
typedef enum { CTL_PEER, CTL_HELLO, CTL_STATUS, CTL_INDICATOR, CTL_ERROR } control_kind;
typedef struct {
    control_kind kind;
    uint8_t count;            // messages for the peer
    uint8_t msg[2][250];
    uint8_t len[2];
    const char *error;        // CTL_ERROR: why
} control_action;

// Parses one line (without its line end).
void control_parse(control_state *st, const char *line, control_action *out);

// A key by name (`a`, `f5`, `enter`, `left`) as a HID usage, or a modifier's bit; false if unknown.
bool key_usage(const char *name, size_t n, uint8_t *usage, uint8_t *modifier);
// An ASCII character as a usage and whether Shift is needed (US layout); false if it has no key.
bool ascii_usage(char c, uint8_t *usage, bool *shift);

#endif
