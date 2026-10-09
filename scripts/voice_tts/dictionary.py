#!/usr/bin/env python3
"""Vosk TTS's Russian dictionary as a MINDDIC1 file for `mind::voice::russian` (252, step 3).

    python3 -I dictionary.py VOSK_MODEL_DIR OUT.dic

vosk-tts reads `dictionary` (word, probability, phonemes; 2 million words, 101 MB) and takes the most probable
pronunciation of each word, else its letter-to-sound rules (vosk_tts/g2p.py, Apache-2.0, ported below and in Rust).
The rules give the dictionary's phonemes for 99 % of the words once they know which vowels are stressed and which
е is ё. So the file stores, per word, only those marks, and the phonemes in full for the rest; each entry is checked
here against the dictionary.

The file, little endian:
    "MINDDIC1", u32 version (1), u32 words, u32 blocks, u32 phonemes
    phonemes: per phoneme u8 length, the name (as the rules and the dictionary spell it), u8 its id in the voice
    blocks: u32 offset of each block from the start of the first block
    blocks of 64 words in byte order, words in Windows-1251: per word u8 bytes shared with the word before it in the
        block (0 for the first), u8 length of the rest, the rest, then u8 n: n < 0x40 marks follow, one byte each
        (bits 0-5 a vowel letter's index among the word's vowels, 0x40 stressed, 0x80 the е is ё); 0xFF: u8 count and
        count phoneme numbers (indices into the table above) follow
    u32 FNV-1a of everything before it
"""
import json
import struct
import sys
from pathlib import Path

# vosk_tts/g2p.py (Apache-2.0, alphacep): stress marks are "+" before a vowel.
SOFTLETTERS = set("яёюиье")
STARTSYL = set("#ъьаяоёуюэеиы-")
OTHERS = {"#", "+", "-", "ь", "ъ"}
SOFTHARD = {"б": "b", "в": "v", "г": "g", "Г": "g", "д": "d", "з": "z", "к": "k", "л": "l", "м": "m", "н": "n",
            "п": "p", "р": "r", "с": "s", "т": "t", "ф": "f", "х": "h"}
OTHER_CONS = {"ж": "zh", "ц": "c", "ч": "ch", "ш": "sh", "щ": "sch", "й": "j"}
VOWELS = {"а": "a", "я": "a", "у": "u", "ю": "u", "о": "o", "ё": "o", "э": "e", "е": "e", "и": "i", "ы": "y"}
VOWEL_LETTERS = "аяуюоёэеиы"
BLOCK = 64


def convert(stressword):
    phones = []
    stress = 0
    for ch in "#" + stressword + "#":
        if ch == "+":
            stress = 1
        else:
            phones.append([ch, stress])
            stress = 0
    for i, (ch, _) in enumerate(phones[:-1]):
        if ch in SOFTHARD:
            phones[i][0] = SOFTHARD[ch] + ("j" if phones[i + 1][0] in SOFTLETTERS else "")
        if ch in OTHER_CONS:
            phones[i][0] = OTHER_CONS[ch]
    out, prev = [], ""
    for ch, s in phones:
        if prev in STARTSYL and ch in "яюеё":
            out.append("j")
        out.append(VOWELS[ch] + str(s) if ch in VOWELS else ch)
        prev = ch
    return [p for p in out if p not in OTHERS]


def marks_for(word, phonemes):
    """The marks that make the rules give `phonemes`, or None."""
    vowels = [i for i, c in enumerate(word) if c in VOWEL_LETTERS]
    sounds = [p for p in phonemes if p[-1] in "01"]
    if len(vowels) != len(sounds) or len(vowels) >= 0x40:
        return None
    chars, marks, stressed = list(word), [], set()
    for k, (i, p) in enumerate(zip(vowels, sounds)):
        mark = 0
        if chars[i] == "е" and p[0] == "o":
            chars[i] = "ё"
            mark |= 0x80
        if p[-1] == "1":
            mark |= 0x40
            stressed.add(i)
        if mark:
            marks.append(k | mark)
    marked = "".join(("+" if i in stressed else "") + c for i, c in enumerate(chars))
    return marks if convert(marked) == phonemes else None


def fnv1a(data):
    h = 0x811C9DC5
    for b in data:
        h = ((h ^ b) * 0x01000193) & 0xFFFFFFFF
    return h


def build(table, best):
    """The file's bytes: `table` maps each phoneme to its id list (config.json's phoneme_id_map), `best` each word to
    its phonemes. Returns (bytes, words, words kept with their phonemes, words left out)."""
    names = [n for n in table if len(table[n]) == 1]
    index = {n: k for k, n in enumerate(names)}
    entries, skipped, explicit = [], 0, 0
    for word, phonemes in best.items():
        try:
            key = word.encode("cp1251")
        except UnicodeEncodeError:
            skipped += 1
            continue
        if len(key) > 255 or any(p not in index for p in phonemes):
            skipped += 1
            continue
        marks = marks_for(word, phonemes)
        if marks is None:
            explicit += 1
            tail = bytes([0xFF, len(phonemes)] + [index[p] for p in phonemes])
        else:
            tail = bytes([len(marks)] + marks)
        entries.append((key, tail))
    entries.sort()
    blocks, offsets, size = [], [], 0
    for start in range(0, len(entries), BLOCK):
        block, prev = bytearray(), b""
        for key, tail in entries[start:start + BLOCK]:
            shared = 0
            while shared < min(len(prev), len(key), 255) and prev[shared] == key[shared]:
                shared += 1
            block += bytes([shared, len(key) - shared]) + key[shared:] + tail
            prev = key
        offsets.append(size)
        size += len(block)
        blocks.append(bytes(block))
    head = bytearray(b"MINDDIC1" + struct.pack("<IIII", 1, len(entries), len(blocks), len(names)))
    for n in names:
        encoded = n.encode()
        head += bytes([len(encoded)]) + encoded + bytes([table[n][0]])
    head += struct.pack(f"<{len(offsets)}I", *offsets)
    body = bytes(head) + b"".join(blocks)
    return body + struct.pack("<I", fnv1a(body)), len(entries), explicit, skipped


def ids(text, table, best):
    """vosk-tts's ids of `text` (Synth.g2p_noembed) with the dictionary `best`, leaving out what the voice lacks."""
    import re
    pattern = '([,.?!;:"() ])'
    names = ["^"]
    for word in re.split(pattern, text.strip().replace("—", "-").lower()):
        if word == "":
            continue
        if re.match(pattern, word) or word == "-":
            names.append(word)
        else:
            names += best[word] if word in best else convert(word)
    out = list(table["^"])
    for n in names[1:] + ["$"]:
        if n in table:
            out += [0] + table[n]
    return out


def main():
    model, out = Path(sys.argv[1]), Path(sys.argv[2])
    table = json.load(open(model / "config.json", encoding="utf-8"))["phoneme_id_map"]
    best, probability = {}, {}
    for line in open(model / "dictionary", encoding="utf-8"):
        word, prob, phonemes = line.split(maxsplit=2)
        if probability.get(word, 0) < float(prob):
            best[word], probability[word] = phonemes.split(), float(prob)
    data, words, explicit, skipped = build(table, best)
    out.write_bytes(data)
    print(f"{out}: {words} words ({explicit} with their phonemes, {skipped} left out), {len(data)} bytes")


if __name__ == "__main__":
    main()
