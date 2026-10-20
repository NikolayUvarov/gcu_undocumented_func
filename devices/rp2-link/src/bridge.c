// The board-to-board frames (bridge.h): plain C, built for the boards and for the host tests.
#include "bridge.h"
#include <string.h>

uint16_t crc16(const uint8_t *data, size_t n) {
    // CRC-16/CCITT-FALSE: polynomial 0x1021, initial 0xFFFF.
    uint16_t crc = 0xFFFF;
    for (size_t i = 0; i < n; i++) {
        crc ^= (uint16_t)data[i] << 8;
        for (int b = 0; b < 8; b++) crc = (crc & 0x8000) ? (uint16_t)(crc << 1 ^ 0x1021) : (uint16_t)(crc << 1);
    }
    return crc;
}

size_t cobs_encode(const uint8_t *in, size_t n, uint8_t *out) {
    size_t code_at = 0, o = 1;
    uint8_t code = 1;
    for (size_t i = 0; i < n; i++) {
        if (in[i] == 0) { out[code_at] = code; code_at = o++; code = 1; continue; }
        out[o++] = in[i];
        if (++code == 0xFF) { out[code_at] = code; code_at = o++; code = 1; }
    }
    out[code_at] = code;
    return o;
}

size_t cobs_decode(const uint8_t *in, size_t n, uint8_t *out) {
    size_t i = 0, o = 0;
    while (i < n) {
        uint8_t code = in[i++];
        if (code == 0 || i + code - 1 > n) return SIZE_MAX;
        for (uint8_t k = 1; k < code; k++) out[o++] = in[i++];
        if (code != 0xFF && i < n) out[o++] = 0;
    }
    return o;
}

size_t bridge_frame(uint8_t type, uint8_t channel, uint32_t offset, const uint8_t *payload, size_t n, uint8_t *out) {
    uint8_t raw[BRIDGE_RAW];
    if (n > BRIDGE_PAYLOAD) n = BRIDGE_PAYLOAD;
    raw[0] = type; raw[1] = channel;
    for (int i = 0; i < 4; i++) raw[2 + i] = (uint8_t)(offset >> (8 * i));
    if (n) memcpy(raw + 6, payload, n);
    uint16_t crc = crc16(raw, 6 + n);
    raw[6 + n] = (uint8_t)crc; raw[7 + n] = (uint8_t)(crc >> 8);
    size_t w = cobs_encode(raw, 8 + n, out);
    out[w++] = 0;
    return w;
}

void bridge_rx_init(bridge_rx *rx) { memset(rx, 0, sizeof *rx); }

bool bridge_rx_push(bridge_rx *rx, uint8_t byte, bridge_msg *msg) {
    if (byte != 0) {
        if (rx->len < sizeof rx->buf) rx->buf[rx->len++] = byte; else rx->overflow = true;
        return false;
    }
    size_t len = rx->len;
    bool overflow = rx->overflow;
    rx->len = 0; rx->overflow = false;
    if (len == 0) return false;
    uint8_t raw[BRIDGE_WIRE];
    size_t n = overflow ? SIZE_MAX : cobs_decode(rx->buf, len, raw);
    if (n == SIZE_MAX || n < 8 || n > BRIDGE_RAW || crc16(raw, n - 2) != (uint16_t)(raw[n - 2] | raw[n - 1] << 8)) { rx->dropped++; return false; }
    msg->type = raw[0]; msg->channel = raw[1];
    msg->offset = (uint32_t)raw[2] | (uint32_t)raw[3] << 8 | (uint32_t)raw[4] << 16 | (uint32_t)raw[5] << 24;
    msg->len = (uint16_t)(n - 8);
    memcpy(msg->payload, raw + 6, msg->len);
    return true;
}

void bridge_credit(bridge_tx_stream *s, uint32_t consumed) {
    uint32_t ahead = consumed - s->consumed, outstanding = s->sent - s->consumed;
    if (ahead <= outstanding) s->consumed = consumed;
}

size_t bridge_accept(bridge_rx_stream *s, uint32_t offset, size_t n, size_t *skip) {
    int32_t gap = (int32_t)(offset - s->next);
    *skip = 0;
    if (gap > 0) { s->lost += (uint32_t)gap; s->next = offset; }  // a frame lost before this one
    else if (gap < 0) {                                            // already had (some of) it
        uint32_t behind = (uint32_t)-gap;
        if (behind >= n) return 0;
        *skip = behind;
    }
    size_t keep = n - *skip;
    s->next += (uint32_t)keep;
    return keep;
}
