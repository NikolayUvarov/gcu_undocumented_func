#!/bin/bash
# new_patch.sh — создаёт следующий по номеру патч, принимает его текст со
# стандартного ввода и сразу применяет.
#
# Использование:
#   ./new_patch.sh              создать patch_<max+1>.sh, ввести текст, применить
#   ./new_patch.sh -t           начать с шаблона-заголовка (SCRIPT_DIR/ROOT_DIR)
#   ./new_patch.sh -n           только создать файл, не запускать
#
# Текст патча вводится до Ctrl-D. Пустой ввод отменяет создание.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

USE_TEMPLATE=0
RUN_AFTER=1

while [ $# -gt 0 ]; do
    case "$1" in
        -t|--template) USE_TEMPLATE=1 ;;
        -n|--no-run)   RUN_AFTER=0 ;;
        -h|--help)     sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \?//'; exit 0 ;;
        *) echo "Неизвестный аргумент: $1" >&2; exit 1 ;;
    esac
    shift
done

# --- Определяем максимальный номер ---------------------------------------

max=0
for f in "$SCRIPT_DIR"/patch_*.sh; do
    [ -e "$f" ] || continue
    n=$(basename "$f" | sed -n 's/^patch_0*\([0-9]\+\).*/\1/p')
    [ -n "$n" ] || continue
    [ "$n" -gt "$max" ] && max=$n
done

# Если последний патч пуст — переиспользуем его номер вместо нового.
reuse=""
if [ "$max" -gt 0 ]; then
    last=$(printf '%s/patch_%03d' "$SCRIPT_DIR" "$max")
    for cand in "$last".sh "$last"_*.sh; do
        if [ -f "$cand" ] && [ ! -s "$cand" ]; then
            reuse="$cand"
            break
        fi
    done
fi

if [ -n "$reuse" ]; then
    NUM=$max
    PATCH_FILE="$reuse"
    echo "Найден пустой патч $(basename "$PATCH_FILE") — заполняем его."
else
    NUM=$((max + 1))
    PATCH_FILE=$(printf '%s/patch_%03d.sh' "$SCRIPT_DIR" "$NUM")
fi

if [ -s "$PATCH_FILE" ]; then
    echo "Ошибка: $PATCH_FILE уже существует и непуст." >&2
    exit 1
fi

# --- Приглашение и ввод ---------------------------------------------------

printf 'Максимальный номер патча: %03d\n' "$max"
printf 'Новый патч: %s\n' "$(basename "$PATCH_FILE")"
echo "Вводите текст патча. Завершение — Ctrl-D, отмена — пустой ввод."
echo "----------------------------------------------------------------"
printf 'cat > %s\n' "$PATCH_FILE"

TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT

if [ "$USE_TEMPLATE" = 1 ]; then
    cat > "$TMP" <<TEMPLATE
#!/bin/bash
set -e

SCRIPT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="\$(cd "\$SCRIPT_DIR/.." && pwd)"

echo ">>> Применяем Патч $(printf '%03d' "$NUM"): ..."

TEMPLATE
fi

cat >> "$TMP"

echo "----------------------------------------------------------------"

# Порог "пусто": без шаблона — ноль байт, с шаблоном — только сам шаблон.
min_lines=0
[ "$USE_TEMPLATE" = 1 ] && min_lines=8

if [ "$(wc -l < "$TMP")" -le "$min_lines" ]; then
    echo "Содержимое не введено — патч не создан."
    exit 0
fi

cp "$TMP" "$PATCH_FILE"
echo "Записано: $PATCH_FILE ($(wc -l < "$PATCH_FILE") строк)"

# --- Проверка синтаксиса и запуск ----------------------------------------

if ! bash -n "$PATCH_FILE"; then
    echo "Синтаксическая ошибка в патче — запуск отменён." >&2
    exit 1
fi

if [ "$RUN_AFTER" = 0 ]; then
    echo "Запуск пропущен (-n). Применить вручную: bash $PATCH_FILE"
    exit 0
fi

echo ">>> Запуск $(basename "$PATCH_FILE") из $ROOT_DIR ..."
cd "$ROOT_DIR"
bash "$PATCH_FILE"
