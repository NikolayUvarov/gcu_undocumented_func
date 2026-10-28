// The serial line to the other board (800-LNK-0005): two PIO state machines on LINK_TX_PIN and LINK_RX_PIN, served
// by core 1, which moves bytes between them and two rings that core 0 reads and fills.
#ifndef LINE_H
#define LINE_H
#include <stddef.h>
#include <stdint.h>

void line_start(uint32_t baud);
// Bytes queued to send; how many fitted.
size_t line_write(const uint8_t *data, size_t n);
size_t line_room(void);
// Bytes received since the last call.
size_t line_read(uint8_t *out, size_t n);
// Bytes core 1 could not keep because core 0 was behind.
uint32_t line_overruns(void);
#endif
