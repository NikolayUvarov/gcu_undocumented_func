# page.py <work> <out.html>: the listening page: every chosen голос reading the four listening sentences, with its measurements.
import base64, html, json, subprocess, sys
from pathlib import Path

root, out_path = Path(sys.argv[1]), Path(sys.argv[2])
SYSTEMS = {
    "ru": [
        ("piper-ru_RU-irina-medium", "Piper · irina", "compact", "VITS", "63 МБ", "MIT; данные голоса из RHVoice, лицензия не указана"),
        ("piper-ru_RU-dmitri-medium", "Piper · dmitri", "compact", "VITS", "63 МБ", "MIT; данные голоса CC0"),
        ("vosk-tts-0.7-0", "Vosk TTS 0.7 · голос 0", "compact", "по типу VITS, 5 голосов", "247 МБ со словарём ударений", "Apache-2.0"),
        ("vosk-tts-0.7-3", "Vosk TTS 0.7 · голос 3", "compact", "по типу VITS, 5 голосов", "247 МБ со словарём ударений", "Apache-2.0"),
        ("vosk-tts-0.9-0", "Vosk TTS 0.9 · голос 0", "quality", "диффузионный трансформер и BERT для интонации, 5 голосов", "937 МБ", "Apache-2.0"),
        ("vosk-tts-0.9-2", "Vosk TTS 0.9 · голос 2", "quality", "диффузионный трансформер и BERT для интонации, 5 голосов", "937 МБ", "Apache-2.0"),
        ("vosk-tts-0.9-3", "Vosk TTS 0.9 · голос 3", "quality", "диффузионный трансформер и BERT для интонации, 5 голосов", "937 МБ", "Apache-2.0"),
        ("vosk-tts-0.9-4", "Vosk TTS 0.9 · голос 4", "quality", "диффузионный трансформер и BERT для интонации, 5 голосов", "937 МБ", "Apache-2.0"),
        ("espeech-ru", "ESpeech TTS-1 RL-V2", "quality", "F5-TTS и RUAccent для ударений; голос по образцу", "2,7 ГБ", "Apache-2.0"),
        ("qwen3-ru", "Qwen3-TTS 0.6B", "quality", "языковая модель над звуковыми токенами; голос по образцу", "2,5 ГБ", "Apache-2.0"),
        ("chatterbox-ru", "Chatterbox Multilingual", "quality", "языковая модель над звуковыми токенами, 23 языка", "3,0 ГБ", "MIT"),
    ],
    "en": [
        ("kitten-nano-0", "Kitten TTS nano · голос 2 m", "compact", "по типу StyleTTS, 15M параметров", "24 МБ", "Apache-2.0"),
        ("kitten-mini-1", "Kitten TTS mini · голос 2 f", "compact", "по типу StyleTTS, 80M параметров", "166 МБ", "Apache-2.0"),
        ("piper-en_US-lessac-high", "Piper · lessac", "compact", "VITS", "114 МБ", "MIT; данные голоса под исследовательской лицензией Blizzard 2013"),
        ("piper-en_US-ryan-high", "Piper · ryan", "compact", "VITS", "121 МБ", "MIT; данные голоса CC BY-NC-SA 4.0"),
        ("kokoro-af_heart", "Kokoro-82M · af_heart", "quality", "StyleTTS 2 с декодером iSTFTNet", "354 МБ (168 МБ в int8)", "Apache-2.0"),
        ("kokoro-am_michael", "Kokoro-82M · am_michael", "quality", "StyleTTS 2 с декодером iSTFTNet", "354 МБ (168 МБ в int8)", "Apache-2.0"),
        ("kokoro-bf_emma", "Kokoro-82M · bf_emma", "quality", "StyleTTS 2 с декодером iSTFTNet", "354 МБ (168 МБ в int8)", "Apache-2.0"),
        ("pocket-en", "Pocket TTS · alba", "quality", "Kyutai, 100M параметров", "219 МБ", "CC BY 4.0"),
        ("qwen3-en", "Qwen3-TTS 0.6B", "quality", "языковая модель над звуковыми токенами; голос по образцу", "2,5 ГБ", "Apache-2.0"),
        ("chatterbox-en", "Chatterbox Multilingual", "quality", "языковая модель над звуковыми токенами, 23 языка", "3,0 ГБ", "MIT"),
    ],
}
common = json.load(open(root / "results/common.json"))  # CER and UTMOS on the eight sentences every voice read (common.py)
texts = {r[0]: r[2] for r in (l.rstrip("\n").split("\t") for l in open(root / "sentences.tsv", encoding="utf-8"))}


def mp3(wav):
    data = subprocess.run(["ffmpeg", "-v", "error", "-i", str(wav), "-ac", "1", "-ar", "22050", "-b:a", "56k", "-f", "mp3", "-"],
                          capture_output=True, check=True).stdout
    return "data:audio/mpeg;base64," + base64.b64encode(data).decode()


def num(v, fmt):
    return fmt.format(v) if isinstance(v, (int, float)) else "—"


sections = []
for lang, systems in SYSTEMS.items():
    present = [s for s in systems if (root / "out" / s[0] / "timing.json").exists()]
    rows = []
    for sid, name, variant, kind, size, licence in present:
        timing = root / "out" / f"{sid}-timing" / "timing.json"  # a timing run on a quiet host, when there is one
        t = json.load(open(timing if timing.exists() else root / "out" / sid / "timing.json"))
        c = common.get(sid, {})
        rows.append(f"""<tr><th scope="row">{html.escape(name)}<span class="kind">{html.escape(kind)}</span></th>
<td><span class="pill {variant}">{'компактный' if variant == 'compact' else 'качество'}</span></td>
<td class="n">{num(c.get('cer8'), '{:.2f} %')}{'<span class="thr">4 фразы</span>' if c.get('n') == 4 else ''}</td><td class="n">{num(c.get('utmos8'), '{:.2f}')}</td>
<td class="n">{num(t.get('rtf'), '{:.2f}')}<span class="thr">{t.get('threads', 1)}&nbsp;пот.</span></td>
<td>{html.escape(size)}</td><td class="lic">{html.escape(licence)}</td></tr>""")
    blocks = []
    for i in range(1, 5):
        key = f"{lang}-L{i}"
        voices = []
        for sid, name, variant, *_ in present:
            wav = root / "out" / sid / f"{key}.wav"
            if wav.exists():
                voices.append(f"""<li><span class="vname">{html.escape(name)}</span><span class="pill {variant}">{'компактный' if variant == 'compact' else 'качество'}</span>
<audio controls preload="none" src="{mp3(wav)}"></audio></li>""")
        blocks.append(f"""<section class="phrase"><p class="said">«{html.escape(texts[key])}»</p><ul class="voices">{''.join(voices)}</ul></section>""")
    title = "Русский" if lang == "ru" else "English"
    sections.append(f"""<section class="lang" id="{lang}"><h2>{title}</h2>
<div class="scroll"><table><thead><tr><th>Голос</th><th>Вариант</th><th class="n">CER</th><th class="n">UTMOS</th><th class="n">Время на 1&nbsp;с речи</th><th>Размер</th><th>Лицензия</th></tr></thead>
<tbody>{''.join(rows)}</tbody></table></div>
{''.join(blocks)}</section>""")

page = open(Path(__file__).with_name("page_template.html"), encoding="utf-8").read().replace("<!--SECTIONS-->", "\n".join(sections))
out_path.write_text(page, encoding="utf-8")
print(out_path, round(out_path.stat().st_size / 1e6, 2), "MB")
