#!/usr/bin/env python3
"""Извлекает словоформы с ударением из выгрузки словаря OpenRussian (CC BY-SA 4.0) для scripts/stress_exceptions.py.

CSV (nouns, adjectives, verbs, others) — https://github.com/Badestrand/russian-dictionary. Отбор — по частотному списку
словоформ «слово частота» (например, ru_50k.txt из hermitdave/FrequencyWords): берутся первые N его слов.
Написания сравниваются без различия е/ё. Написание пропускается, если у одной леммы оно читается по-разному или другое
чтение даёт лемма, чья начальная форма встречается хотя бы в 1/20 случаев от самой частой леммы с этим написанием (замок, дома).
    python3 scripts/stress_openrussian.py <каталог с CSV> ru_50k.txt [N] > forms.txt && python3 scripts/stress_exceptions.py forms.txt
"""
import collections
import csv
from pathlib import Path
import sys

VOWELS = "аеёиоуыэюя"
TABLES = ["nouns", "adjectives", "verbs", "others"]
SKIP = {"bare", "translations_en", "translations_de", "gender", "partner", "animate", "indeclinable", "sg_only", "pl_only", "aspect"}


# «доро'га» -> («дорога», 1); None для форм без ударения, с двумя ударениями или с посторонними знаками.
def parse(form):
    form = form.strip().lower().replace("`", "'").replace("\u0301", "'")
    word = form.replace("'", "")
    if not word or any(c not in "абвгдеёжзийклмнопрстуфхцчшщъыьэюя" for c in word):
        return None
    vowels = sum(1 for c in word if c in VOWELS)
    marks = [i for i, c in enumerate(form) if c == "'"]
    if len(marks) > 1 or vowels < 2:
        return None
    if marks:
        if marks[0] == 0 or form[marks[0] - 1] not in VOWELS:
            return None
        return word, sum(1 for c in form[:marks[0]] if c in VOWELS) - 1
    if word.count("ё") == 1:
        return word, sum(1 for c in word[:word.index("ё")] if c in VOWELS)
    return None


def forms(row):
    for key, cell in row.items():
        if key is None or key in SKIP or not cell:
            continue
        for item in cell.replace(";", ",").replace("/", ",").split(","):
            item = item.strip().strip("()*")
            if item and " " not in item and (parsed := parse(item)):
                yield parsed


def fold(word):
    return word.replace("ё", "е")


def resolve(root, frequency, limit):
    """Словарь «написание без ё -> (форма с ё, номер ударного гласного)» для первых `limit` слов частотного списка."""
    # Вес леммы — частота её точного написания (ё в субтитрах редка, поэтому «небо» перевешивает «нёбо»).
    counts = collections.Counter()
    for line in Path(frequency).read_text(encoding="utf-8").splitlines():
        word, _, count = line.partition(" ")
        counts[word] += int(count or 0)
    folded = collections.Counter()
    for word, count in counts.items():
        folded[fold(word)] += count
    wanted = {word for word, _ in folded.most_common(limit)}
    seen = collections.defaultdict(lambda: collections.defaultdict(set))
    for table in TABLES:
        with open(root / f"{table}.csv", encoding="utf-8", newline="") as f:
            for row in csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE):
                lemma = (table, row["bare"], row["accented"])
                for word, index in forms(row):
                    if fold(word) in wanted:
                        seen[fold(word)][lemma].add((word, index))
    result = {}
    for key, lemmas in seen.items():
        if any(len(readings) > 1 for readings in lemmas.values()):
            continue
        weight = collections.Counter()
        for (table, bare, accented), readings in lemmas.items():
            reading = next(iter(readings))
            weight[reading] = max(weight[reading], counts[bare] + 1)
        (best, top), *rest = weight.most_common()
        if all(other * 20 < top for _, other in rest):
            result[key] = best
    return result


def main():
    root, frequency = Path(sys.argv[1]), sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 50000
    for word, index in sorted(resolve(root, frequency, limit).values()):
        at = [i for i, c in enumerate(word) if c in VOWELS][index]
        print(word[:at] + word[at].upper() + word[at + 1:])


if __name__ == "__main__":
    main()
