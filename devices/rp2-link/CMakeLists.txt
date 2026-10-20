# The MIND Link multi-device's firmware (800-LNK-0005), for RP2040 and RP2350 boards: build.sh builds every board.
# LINK_BOARD picks the board: rp2040-zero, rp2040-yd, rp2040-pico, rp2350-zero or rp2350-pico2.
cmake_minimum_required(VERSION 3.13)

set(LINK_BOARD "rp2040-zero" CACHE STRING "Board kind")
# The two wires to the other board, and the line's rate (bit/s); docs/link/multi-device.md.
set(LINK_TX_PIN 4 CACHE STRING "GPIO sending to the other board")
set(LINK_RX_PIN 5 CACHE STRING "GPIO receiving from the other board")
set(LINK_BAUD 15000000 CACHE STRING "Rate between the boards")

if(LINK_BOARD STREQUAL "rp2040-zero")
    set(PICO_BOARD waveshare_rp2040_zero)
elseif(LINK_BOARD STREQUAL "rp2040-yd")
    # The YD-RP2040: a Pico's layout with a WS2812 on GP23.
    set(PICO_BOARD pico)
    set(LINK_WS2812_PIN 23)
elseif(LINK_BOARD STREQUAL "rp2040-pico")
    set(PICO_BOARD pico)
elseif(LINK_BOARD STREQUAL "rp2350-zero")
    set(PICO_BOARD waveshare_rp2350_zero)
elseif(LINK_BOARD STREQUAL "rp2350-pico2")
    set(PICO_BOARD pico2)
else()
    message(FATAL_ERROR "Unknown LINK_BOARD: ${LINK_BOARD}")
endif()

if(NOT DEFINED ENV{PICO_SDK_PATH})
    message(FATAL_ERROR "PICO_SDK_PATH is not set: build.sh sets it from the toolchain's env.sh")
endif()
include($ENV{PICO_SDK_PATH}/pico_sdk_init.cmake)

project(rp2_link C CXX ASM)
set(CMAKE_C_STANDARD 11)
pico_sdk_init()

add_executable(rp2_link
    src/main.c src/usb_descriptors.c src/line.c src/bridge.c src/control.c src/led.c)
pico_generate_pio_header(rp2_link ${CMAKE_CURRENT_LIST_DIR}/src/line.pio)
target_include_directories(rp2_link PRIVATE src)
target_compile_options(rp2_link PRIVATE -Wall -Wextra)
target_compile_definitions(rp2_link PRIVATE
    LINK_TX_PIN=${LINK_TX_PIN} LINK_RX_PIN=${LINK_RX_PIN} LINK_BAUD=${LINK_BAUD} LINK_BOARD_NAME="${LINK_BOARD}")
if(DEFINED LINK_WS2812_PIN)
    target_compile_definitions(rp2_link PRIVATE PICO_DEFAULT_WS2812_PIN=${LINK_WS2812_PIN})
endif()
target_link_libraries(rp2_link PRIVATE pico_stdlib pico_multicore pico_unique_id pico_status_led hardware_pio
    hardware_clocks tinyusb_device tinyusb_board)
pico_add_extra_outputs(rp2_link)
