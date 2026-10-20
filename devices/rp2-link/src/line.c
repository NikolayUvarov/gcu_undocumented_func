// The line to the other board (line.h). Each ring has one writer and one reader, on different cores: the writer
// publishes its index after the data, behind a memory barrier.
#include "line.h"
#include "line.pio.h"
#include "hardware/clocks.h"
#include "hardware/pio.h"
#include "hardware/sync.h"
#include "pico/multicore.h"

enum { RX_RING = 8192, TX_RING = 4096 };
static uint8_t rx_ring[RX_RING], tx_ring[TX_RING];
static volatile uint32_t rx_head, rx_tail, tx_head, tx_tail, overruns;
static PIO pio;
static uint tx_sm, rx_sm;

static void core1(void) {
    for (;;) {
        while (!pio_sm_is_rx_fifo_empty(pio, rx_sm)) {
            uint8_t byte = (uint8_t)(pio->rxf[rx_sm] >> 24);
            uint32_t head = rx_head;
            if (head - rx_tail == RX_RING) { overruns++; continue; }
            rx_ring[head % RX_RING] = byte;
            __dmb();
            rx_head = head + 1;
        }
        while (tx_tail != tx_head && !pio_sm_is_tx_fifo_full(pio, tx_sm)) {
            uint32_t tail = tx_tail;
            __dmb();
            pio_sm_put(pio, tx_sm, tx_ring[tail % TX_RING]);
            tx_tail = tail + 1;
        }
    }
}

void line_start(uint32_t baud) {
    pio = pio0;
    float div = (float)clock_get_hz(clk_sys) / (8.0f * (float)baud);

    tx_sm = (uint)pio_claim_unused_sm(pio, true);
    uint tx_offset = pio_add_program(pio, &line_tx_program);
    pio_sm_set_pins_with_mask(pio, tx_sm, 1u << LINK_TX_PIN, 1u << LINK_TX_PIN);
    pio_sm_set_pindirs_with_mask(pio, tx_sm, 1u << LINK_TX_PIN, 1u << LINK_TX_PIN);
    pio_gpio_init(pio, LINK_TX_PIN);
    pio_sm_config tx = line_tx_program_get_default_config(tx_offset);
    sm_config_set_out_shift(&tx, true, false, 32);
    sm_config_set_out_pins(&tx, LINK_TX_PIN, 1);
    sm_config_set_sideset_pins(&tx, LINK_TX_PIN);
    sm_config_set_fifo_join(&tx, PIO_FIFO_JOIN_TX);
    sm_config_set_clkdiv(&tx, div);
    pio_sm_init(pio, tx_sm, tx_offset, &tx);

    rx_sm = (uint)pio_claim_unused_sm(pio, true);
    uint rx_offset = pio_add_program(pio, &line_rx_program);
    pio_sm_set_consecutive_pindirs(pio, rx_sm, LINK_RX_PIN, 1, false);
    pio_gpio_init(pio, LINK_RX_PIN);
    // Idle high with no other board on the wire, so noise is not read as start bits.
    gpio_pull_up(LINK_RX_PIN);
    pio_sm_config rx = line_rx_program_get_default_config(rx_offset);
    sm_config_set_in_pins(&rx, LINK_RX_PIN);
    sm_config_set_in_shift(&rx, true, false, 32);
    sm_config_set_fifo_join(&rx, PIO_FIFO_JOIN_RX);
    sm_config_set_clkdiv(&rx, div);
    pio_sm_init(pio, rx_sm, rx_offset, &rx);

    pio_sm_set_enabled(pio, tx_sm, true);
    pio_sm_set_enabled(pio, rx_sm, true);
    multicore_launch_core1(core1);
}

size_t line_room(void) { return TX_RING - (tx_head - tx_tail); }

size_t line_write(const uint8_t *data, size_t n) {
    uint32_t head = tx_head;
    size_t room = TX_RING - (head - tx_tail);
    if (n > room) n = room;
    for (size_t i = 0; i < n; i++) tx_ring[(head + i) % TX_RING] = data[i];
    __dmb();
    tx_head = head + (uint32_t)n;
    return n;
}

size_t line_read(uint8_t *out, size_t n) {
    uint32_t tail = rx_tail, avail = rx_head - tail;
    __dmb();
    if (n > avail) n = avail;
    for (size_t i = 0; i < n; i++) out[i] = rx_ring[(tail + i) % RX_RING];
    __dmb();
    rx_tail = tail + (uint32_t)n;
    return n;
}

uint32_t line_overruns(void) { return overruns; }
