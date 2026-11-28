// The browser's key codes (KeyboardEvent.code) as evdev's (Linux input-event-codes.h), which virtio-input carries.
const rows = [
    ['Escape', 1], ['Minus', 12], ['Equal', 13], ['Backspace', 14], ['Tab', 15], ['BracketLeft', 26], ['BracketRight', 27],
    ['Enter', 28], ['ControlLeft', 29], ['Semicolon', 39], ['Quote', 40], ['Backquote', 41], ['ShiftLeft', 42], ['Backslash', 43],
    ['Comma', 51], ['Period', 52], ['Slash', 53], ['ShiftRight', 54], ['NumpadMultiply', 55], ['AltLeft', 56], ['Space', 57],
    ['CapsLock', 58], ['NumLock', 69], ['ScrollLock', 70], ['Numpad7', 71], ['Numpad8', 72], ['Numpad9', 73], ['NumpadSubtract', 74],
    ['Numpad4', 75], ['Numpad5', 76], ['Numpad6', 77], ['NumpadAdd', 78], ['Numpad1', 79], ['Numpad2', 80], ['Numpad3', 81],
    ['Numpad0', 82], ['NumpadDecimal', 83], ['IntlBackslash', 86], ['F11', 87], ['F12', 88], ['NumpadEnter', 96], ['ControlRight', 97],
    ['NumpadDivide', 98], ['PrintScreen', 99], ['AltRight', 100], ['Home', 102], ['ArrowUp', 103], ['PageUp', 104], ['ArrowLeft', 105],
    ['ArrowRight', 106], ['End', 107], ['ArrowDown', 108], ['PageDown', 109], ['Insert', 110], ['Delete', 111], ['Pause', 119],
    ['MetaLeft', 125], ['MetaRight', 126], ['ContextMenu', 127],
];
const letters = { Q: 16, W: 17, E: 18, R: 19, T: 20, Y: 21, U: 22, I: 23, O: 24, P: 25, A: 30, S: 31, D: 32, F: 33, G: 34, H: 35, J: 36,
    K: 37, L: 38, Z: 44, X: 45, C: 46, V: 47, B: 48, N: 49, M: 50 };
export const KEYS = new Map([
    ...rows,
    ...Object.entries(letters).map(([letter, code]) => [`Key${letter}`, code]),
    ...[1, 2, 3, 4, 5, 6, 7, 8, 9, 0].map((digit, i) => [`Digit${digit}`, 2 + i]),
    ...Array.from({ length: 10 }, (_, i) => [`F${i + 1}`, 59 + i]),
]);
