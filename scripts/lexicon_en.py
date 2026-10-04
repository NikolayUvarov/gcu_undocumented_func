#!/usr/bin/env python3
"""Builds phonetics/data/lexicon_en.txt: pronunciations of frequent English words from CMUdict (BSD, github.com/cmusphinx/cmudict).

Selection uses a frequency list of "word count" lines (e.g. en_50k.txt from hermitdave/FrequencyWords): the first N words.
Notation is the tts lexicon alphabet (phonetics/src/text.rs): a o u e i = vowels, @ = schwa, & = [æ], ' = stress before a vowel,
I U A R = [ɪ ʊ ʌ ɝ], S Z C T D = [ʃ ʒ tʃ θ ð], j = [j], h = aspiration. Apostrophes are dropped from words (as the synthesizer reads them: don't -> dont).
    python3 scripts/lexicon_en.py cmudict.dict en_50k.txt [N]
"""
from pathlib import Path
import sys

LEXICON = Path(__file__).resolve().parents[1] / "phonetics/data/lexicon_en.txt"
# ARPAbet -> tts alphabet; the synthesizer has no [ŋ]: n. Stressed AH = [ʌ], unstressed = schwa.
PHONES = {
    "AA": "a", "AE": "&", "AH": "@", "AO": "o", "AW": "au", "AY": "aj", "EH": "e", "ER": "R", "EY": "ej", "IH": "I", "IY": "i",
    "OW": "ou", "OY": "oj", "UH": "U", "UW": "u", "B": "b", "CH": "C", "D": "d", "DH": "D", "F": "f", "G": "g", "HH": "h",
    "JH": "dZ", "K": "k", "L": "l", "M": "m", "N": "n", "NG": "n", "P": "p", "R": "r", "S": "s", "SH": "S", "T": "t",
    "TH": "T", "V": "v", "W": "w", "Y": "j", "Z": "z", "ZH": "Z",
}


def code(phones):
    out = []
    for phone in phones:
        base, stress = phone.rstrip("012"), phone[len(phone.rstrip("012")):]
        sound = "A" if base == "AH" and stress == "1" else PHONES[base]
        out.append(("'" if stress == "1" else "") + sound)
    return "".join(out)


def main():
    cmu, frequency = sys.argv[1], sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 20000
    pronunciations = {}
    for line in Path(cmu).read_text(encoding="utf-8", errors="replace").splitlines():
        word, *phones = line.split("#")[0].split()
        if phones and "(" not in word:
            pronunciations.setdefault(word, phones)
    entries = {}
    for line in Path(frequency).read_text(encoding="utf-8").splitlines()[:limit]:
        word = line.split()[0]
        key = word.replace("'", "")
        if word not in pronunciations or not key.isascii() or not key.isalpha():
            continue
        # "we'll" and "well" collide once the apostrophe is removed: the word without an apostrophe wins.
        if key in entries and "'" in word:
            continue
        entries[key] = code(pronunciations[word])
    header = "# Pronunciations of frequent English words (CMUdict, BSD; github.com/cmusphinx/cmudict); built by scripts/lexicon_en.py."
    LEXICON.write_text("\n".join([header, *(f"{w} {c}" for w, c in sorted(entries.items()))]) + "\n", encoding="utf-8")
    print(f"{len(entries)} words in {LEXICON}")


if __name__ == "__main__":
    main()
