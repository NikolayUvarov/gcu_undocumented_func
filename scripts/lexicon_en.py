#!/usr/bin/env python3
"""Строит tts/data/lexicon_en.txt — произношения частых английских слов из CMUdict (BSD, github.com/cmusphinx/cmudict).

Отбор — по частотному списку словоформ «слово частота» (например, en_50k.txt из hermitdave/FrequencyWords): первые N слов.
Запись — алфавит лексикона tts (tts/src/text.rs): a o u e i — гласные, @ — шва, & — [æ], ' — ударение перед гласным,
I U A R — [ɪ ʊ ʌ ɝ], S Z C T D — ш ж ч θ ð, j — [й], h — придыхание. Апостроф из слова убирается (так его читает синтезатор: don't -> dont).
    python3 scripts/lexicon_en.py cmudict.dict en_50k.txt [N]
"""
from pathlib import Path
import sys

LEXICON = Path(__file__).resolve().parents[1] / "tts/data/lexicon_en.txt"
# ARPAbet -> алфавит tts; [ŋ] синтезатор не различает: n. Ударный AH — [ʌ], безударный — шва.
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
        # «we'll» и «well» после удаления апострофа совпадают: слово без апострофа важнее.
        if key in entries and "'" in word:
            continue
        entries[key] = code(pronunciations[word])
    header = "# Произношения частых английских слов (CMUdict, BSD; github.com/cmusphinx/cmudict); строится scripts/lexicon_en.py."
    LEXICON.write_text("\n".join([header, *(f"{w} {c}" for w, c in sorted(entries.items()))]) + "\n", encoding="utf-8")
    print(f"{len(entries)} слов в {LEXICON}")


if __name__ == "__main__":
    main()
