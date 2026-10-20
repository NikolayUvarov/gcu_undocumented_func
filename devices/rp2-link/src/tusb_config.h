// TinyUSB for the multi-device (800-LNK-0005): a full-speed device with three CDC-ACM ports and three HID interfaces.
#ifndef TUSB_CONFIG_H
#define TUSB_CONFIG_H

#define CFG_TUSB_RHPORT0_MODE (OPT_MODE_DEVICE | OPT_MODE_FULL_SPEED)
#define CFG_TUD_ENABLED 1
#define CFG_TUD_ENDPOINT0_SIZE 64

#define CFG_TUD_CDC 3
#define CFG_TUD_HID 3
#define CFG_TUD_MSC 0
#define CFG_TUD_MIDI 0
#define CFG_TUD_VENDOR 0

#define CFG_TUD_CDC_RX_BUFSIZE 1024
#define CFG_TUD_CDC_TX_BUFSIZE 1024
#define CFG_TUD_CDC_EP_BUFSIZE 64
#define CFG_TUD_HID_EP_BUFSIZE 16

#endif
