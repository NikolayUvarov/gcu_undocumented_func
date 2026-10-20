// The link between the two boards (800-LNK-0005): frames over a byte line, each COBS-encoded and ended by a zero
// byte: type, channel, a stream offset, the payload and CRC-16/CCITT. Data channels carry their bytes in order;
// the receiver's credits (how far it has consumed each stream) bound what the sender may send, so neither board's
// buffers overflow. A damaged frame is dropped and counted; its bytes are lost and skipped, never waited for.
#ifndef BRIDGE_H
#define BRIDGE_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum { CH_SERIAL = 0, CH_DATA = 1, CH_CONTROL = 2, CHANNELS = 3 };
enum { FT_HELLO = 1, FT_DATA = 2, FT_CREDIT = 3 };
enum { BRIDGE_PAYLOAD = 256, BRIDGE_WINDOW = 4096 };
// type, channel, offset (4) + payload + crc (2), COBS overhead, the zero.
enum { BRIDGE_RAW = 6 + BRIDGE_PAYLOAD + 2, BRIDGE_WIRE = BRIDGE_RAW + BRIDGE_RAW / 254 + 2 };

uint16_t crc16(const uint8_t *data, size_t n);
size_t cobs_encode(const uint8_t *in, size_t n, uint8_t *out);
// Decodes `n` bytes (no zero among them) into `out` (room for `n`); the length, or SIZE_MAX if malformed.
size_t cobs_decode(const uint8_t *in, size_t n, uint8_t *out);

// One frame: `out` gets the wire bytes, zero included (room for BRIDGE_WIRE); the count.
size_t bridge_frame(uint8_t type, uint8_t channel, uint32_t offset, const uint8_t *payload, size_t n, uint8_t *out);

typedef struct { uint8_t type, channel; uint32_t offset; uint16_t len; uint8_t payload[BRIDGE_PAYLOAD]; } bridge_msg;

// Gathers wire bytes; `push` returns true when a whole, checked frame is in `msg`.
typedef struct { uint8_t buf[BRIDGE_WIRE]; size_t len; bool overflow; uint32_t dropped; } bridge_rx;
void bridge_rx_init(bridge_rx *rx);
bool bridge_rx_push(bridge_rx *rx, uint8_t byte, bridge_msg *msg);

// One direction of one channel, as its sender sees it: bytes sent, and how far the receiver has consumed.
typedef struct { uint32_t sent, consumed; } bridge_tx_stream;
// What the sender may send now within the window.
static inline uint32_t bridge_room(const bridge_tx_stream *s) {
    uint32_t used = s->sent - s->consumed;
    return used >= BRIDGE_WINDOW ? 0 : BRIDGE_WINDOW - used;
}
// A credit from the receiver: its consumed offset, taken only if it moves forward (wrapping) and not past `sent`.
void bridge_credit(bridge_tx_stream *s, uint32_t consumed);

// One channel as its receiver sees it: the next offset expected, and the bytes it lost to damaged frames.
typedef struct { uint32_t next; uint32_t lost; } bridge_rx_stream;
// A data frame at `offset` with `n` bytes: how many of them to keep (from `*skip` on); a frame from the past is
// dropped (a repeat), one from the future means the bytes between were lost.
size_t bridge_accept(bridge_rx_stream *s, uint32_t offset, size_t n, size_t *skip);

// The other board's boot number from its hello (a random number drawn at its start): true when it changed, so this
// board must start both directions of every channel afresh; `*known` keeps the last one (0: none yet).
bool bridge_peer_restarted(uint32_t *known, uint32_t boot);

#endif
