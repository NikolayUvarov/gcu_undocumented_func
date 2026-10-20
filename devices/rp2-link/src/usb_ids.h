// The multi-device's USB identity and interface instances (800-LNK-0005).
#ifndef USB_IDS_H
#define USB_IDS_H
// pid.codes' test vendor and product ID, for development: a product ID of its own before others use it.
#define LINK_USB_VENDOR 0x1209
#define LINK_USB_PRODUCT 0x0001
// The CDC instances, in the order of the configuration: the serial port, the fast data port, the control port.
enum { CDC_SERIAL = 0, CDC_DATA = 1, CDC_CONTROL = 2 };
// The HID instances.
enum { HID_KEYBOARD = 0, HID_MOUSE = 1, HID_TABLET = 2 };
#endif
