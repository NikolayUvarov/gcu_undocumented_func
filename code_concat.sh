#!/bin/bash
# code_contat.sh — рекурсивно обходит каталог и склеивает все текстовые файлы
# в один txt для передачи в контекст модели.
#
# Использование:
#   ./code_contat.sh                      # обойти текущий каталог -> code_context.txt
#   ./code_contat.sh <каталог>            # обойти указанный каталог
#   ./code_contat.sh <каталог> <выход.txt>
#
# Переменные окружения:
#   EXCLUDE_DIRS="dir1 dir2"   дополнительные каталоги к исключению
#   MAX_SIZE=1048576           пропускать файлы крупнее (байт), по умолчанию 1 МиБ

set -euo pipefail

SRC_DIR=${1:-.}
OUT_FILE=${2:-code_context.txt}
MAX_SIZE=${MAX_SIZE:-1048576}

if [ ! -d "$SRC_DIR" ]; then
    echo "Ошибка: каталог не найден: $SRC_DIR" >&2
    exit 1
fi

SRC_DIR=$(cd "$SRC_DIR" && pwd)

# Выход кладём по абсолютному пути, чтобы не поймать его же при обходе.
case "$OUT_FILE" in
    /*) : ;;
    *)  OUT_FILE="$(pwd)/$OUT_FILE" ;;
esac

# Каталоги, которые не несут исходного кода.
SKIP_DIRS="
.git .svn .hg
target build dist out
node_modules vendor
__pycache__ .venv venv
.idea .vscode .cache
usb_root
${EXCLUDE_DIRS:-}
"

# Собираем предикат -prune для find.
prune_args=()
for d in $SKIP_DIRS; do
    prune_args+=( -name "$d" -o )
done
unset 'prune_args[${#prune_args[@]}-1]'   # убираем хвостовой -o

: > "$OUT_FILE"

total=0
skipped=0

while IFS= read -r -d '' file; do
    # Сам выходной файл в выборку не берём.
    [ "$file" = "$OUT_FILE" ] && continue

    size=$(stat -c%s "$file" 2>/dev/null || echo 0)
    if [ "$size" -gt "$MAX_SIZE" ]; then
        skipped=$((skipped + 1))
        continue
    fi

    # Бинарные файлы пропускаем: grep -Iq успешен только для текста.
    if ! grep -Iq . "$file" 2>/dev/null && [ "$size" -gt 0 ]; then
        skipped=$((skipped + 1))
        continue
    fi

    rel=${file#"$SRC_DIR"/}
    fname=$(basename "$file")

    {
        printf '=== file: %s\n' "$rel"
        printf '=== file %s content:\n' "$fname"
        cat "$file"
        # Гарантируем перевод строки перед закрывающим маркером.
        [ -n "$(tail -c 1 "$file")" ] && printf '\n'
        printf '=== end of file %s content\n\n' "$fname"
    } >> "$OUT_FILE"

    total=$((total + 1))
done < <(find "$SRC_DIR" \( "${prune_args[@]}" \) -prune -o -type f -print0 | sort -z)

echo "Записано файлов: $total (пропущено бинарных/крупных: $skipped)"
echo "Результат: $OUT_FILE ($(stat -c%s "$OUT_FILE") байт)"
