// The multi-device's USB descriptors (800-LNK-0005): three CDC-ACM ports (serial, data, control), a boot keyboard, a
// boot mouse and an absolute pointer. Every class has a driver in the host systems, so no driver is installed.
#include "tusb.h"
#include "pico/unique_id.h"
#include "usb_ids.h"
#include <string.h>

static const tusb_desc_device_t DEVICE = {
    .bLength = sizeof(tusb_desc_device_t), .bDescriptorType = TUSB_DESC_DEVICE, .bcdUSB = 0x0200,
    // Interface association descriptors group each port's two interfaces.
    .bDeviceClass = TUSB_CLASS_MISC, .bDeviceSubClass = MISC_SUBCLASS_COMMON, .bDeviceProtocol = MISC_PROTOCOL_IAD,
    .bMaxPacketSize0 = CFG_TUD_ENDPOINT0_SIZE, .idVendor = LINK_USB_VENDOR, .idProduct = LINK_USB_PRODUCT, .bcdDevice = 0x0100,
    .iManufacturer = 1, .iProduct = 2, .iSerialNumber = 3, .bNumConfigurations = 1,
};

const uint8_t *tud_descriptor_device_cb(void) { return (const uint8_t *)&DEVICE; }

static const uint8_t KEYBOARD_REPORT[] = { TUD_HID_REPORT_DESC_KEYBOARD() };
static const uint8_t MOUSE_REPORT[] = { TUD_HID_REPORT_DESC_MOUSE() };
// Three buttons, then X and Y from 0 to 32767 across the screen.
static const uint8_t TABLET_REPORT[] = {
    HID_USAGE_PAGE(HID_USAGE_PAGE_DESKTOP), HID_USAGE(HID_USAGE_DESKTOP_MOUSE), HID_COLLECTION(HID_COLLECTION_APPLICATION),
      HID_USAGE(HID_USAGE_DESKTOP_POINTER), HID_COLLECTION(HID_COLLECTION_PHYSICAL),
        HID_USAGE_PAGE(HID_USAGE_PAGE_BUTTON), HID_USAGE_MIN(1), HID_USAGE_MAX(3), HID_LOGICAL_MIN(0), HID_LOGICAL_MAX(1),
        HID_REPORT_COUNT(3), HID_REPORT_SIZE(1), HID_INPUT(HID_DATA | HID_VARIABLE | HID_ABSOLUTE),
        HID_REPORT_COUNT(1), HID_REPORT_SIZE(5), HID_INPUT(HID_CONSTANT),
        HID_USAGE_PAGE(HID_USAGE_PAGE_DESKTOP), HID_USAGE(HID_USAGE_DESKTOP_X), HID_USAGE(HID_USAGE_DESKTOP_Y),
        HID_LOGICAL_MIN(0), HID_LOGICAL_MAX_N(32767, 2), HID_REPORT_SIZE(16), HID_REPORT_COUNT(2),
        HID_INPUT(HID_DATA | HID_VARIABLE | HID_ABSOLUTE),
      HID_COLLECTION_END,
    HID_COLLECTION_END,
};

const uint8_t *tud_hid_descriptor_report_cb(uint8_t instance) {
    return instance == HID_KEYBOARD ? KEYBOARD_REPORT : instance == HID_MOUSE ? MOUSE_REPORT : TABLET_REPORT;
}

enum { ITF_SERIAL, ITF_SERIAL_DATA, ITF_DATA, ITF_DATA_DATA, ITF_CONTROL, ITF_CONTROL_DATA, ITF_KEYBOARD, ITF_MOUSE, ITF_TABLET, ITF_COUNT };
#define CONFIG_LEN (TUD_CONFIG_DESC_LEN + 3 * TUD_CDC_DESC_LEN + 3 * TUD_HID_DESC_LEN)

static const uint8_t CONFIGURATION[] = {
    TUD_CONFIG_DESCRIPTOR(1, ITF_COUNT, 0, CONFIG_LEN, 0x00, 100),
    TUD_CDC_DESCRIPTOR(ITF_SERIAL, 4, 0x81, 8, 0x02, 0x82, 64),
    TUD_CDC_DESCRIPTOR(ITF_DATA, 5, 0x83, 8, 0x04, 0x84, 64),
    TUD_CDC_DESCRIPTOR(ITF_CONTROL, 6, 0x85, 8, 0x06, 0x86, 64),
    TUD_HID_DESCRIPTOR(ITF_KEYBOARD, 7, HID_ITF_PROTOCOL_KEYBOARD, sizeof KEYBOARD_REPORT, 0x87, 8, 5),
    TUD_HID_DESCRIPTOR(ITF_MOUSE, 8, HID_ITF_PROTOCOL_MOUSE, sizeof MOUSE_REPORT, 0x88, 8, 5),
    TUD_HID_DESCRIPTOR(ITF_TABLET, 9, HID_ITF_PROTOCOL_NONE, sizeof TABLET_REPORT, 0x89, 8, 5),
};

const uint8_t *tud_descriptor_configuration_cb(uint8_t index) { (void)index; return CONFIGURATION; }

static const char *const STRINGS[] = {
    NULL, "MIND Core", "MIND Link", NULL, "MIND Link serial", "MIND Link data", "MIND Link control",
    "MIND Link keyboard", "MIND Link mouse", "MIND Link tablet",
};

const uint16_t *tud_descriptor_string_cb(uint8_t index, uint16_t langid) {
    (void)langid;
    static uint16_t out[1 + 32];
    char serial[2 * PICO_UNIQUE_BOARD_ID_SIZE_BYTES + 1];
    const char *s;
    if (index == 0) { out[1] = 0x0409; out[0] = (uint16_t)(TUSB_DESC_STRING << 8 | 4); return out; }
    if (index == 3) { pico_get_unique_board_id_string(serial, sizeof serial); s = serial; }
    else if (index < sizeof STRINGS / sizeof STRINGS[0]) s = STRINGS[index];
    else return NULL;
    size_t n = strlen(s);
    if (n > 32) n = 32;
    for (size_t i = 0; i < n; i++) out[1 + i] = (uint8_t)s[i];
    out[0] = (uint16_t)(TUSB_DESC_STRING << 8 | (2 * n + 2));
    return out;
}
