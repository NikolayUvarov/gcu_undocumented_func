#!/usr/bin/env python3
"""Пополняет словарь ударений tts только словоформами, на которых ошибается эвристика синтезатора.

Вход — файлы со словоформами, где ударная гласная записана заглавной (дОбрый, тебЯ), по нескольку в строке;
строки с # пропускаются. Формы с одинаковым написанием (без различия е/ё) и разным чтением (омографы) не добавляются.
Формы с «ё» сохраняются всегда: по ним синтезатор восстанавливает «ё» в тексте, набранном через «е».
    python3 scripts/stress_exceptions.py my_words.txt
"""
import collections
from pathlib import Path
import sys

VOWELS = "аеёиоуыэюя"
DICTIONARY = Path(__file__).resolve().parents[1] / "tts/data/stress_ru.txt"


def stressed(form):
    upper = [i for i, c in enumerate(form) if c.isupper()]
    if len(upper) != 1 or form[upper[0]].lower() not in VOWELS:
        return None
    return sum(1 for c in form[:upper[0]] if c in VOWELS)


# Та же эвристика, что в tts/src/text.rs: «ё» ударная; на согласный — последний слог, на гласный — предпоследний.
def heuristic(word):
    count = sum(1 for c in word if c in VOWELS)
    if "ё" in word:
        return sum(1 for c in word[:word.index("ё")] if c in VOWELS)
    return 0 if count == 1 else count - 1 if word[-1] not in VOWELS else count - 2


def main():
    forms = collections.defaultdict(set)
    sources = [DICTIONARY, *map(Path, sys.argv[1:])]
    for path in sources:
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.startswith("#"):
                continue
            for form in line.split():
                if stressed(form) is not None:
                    forms[form.lower().replace("ё", "е")].add(form)
    # Порядок по написанию без ё: синтезатор ищет в словаре двоичным поиском.
    entries = []
    for word, variants in forms.items():
        if len(variants) > 1:
            print(f"пропущен омограф: {word} {sorted(variants)}")
            continue
        form = variants.pop()
        if sum(1 for c in word if c in VOWELS) > 1 and (heuristic(form.lower()) != stressed(form) or "ё" in form.lower()):
            entries.append(form)
    header = DICTIONARY.read_text(encoding="utf-8").splitlines()[0]
    DICTIONARY.write_text("\n".join([header, *sorted(entries, key=lambda e: e.lower().replace("ё", "е"))]) + "\n", encoding="utf-8")
    print(f"{len(entries)} исключений в {DICTIONARY}")


if __name__ == "__main__":
    main()
