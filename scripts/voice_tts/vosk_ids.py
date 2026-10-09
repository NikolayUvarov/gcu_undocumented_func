#!/usr/bin/env python3
"""vosk-tts's phoneme ids of the Russian sentences, to compare `mind::voice::russian` with (252, the host only).

    python3 -I vosk_ids.py VOSK_MODEL_DIR sentences.tsv OUT.txt

OUT.txt gets per Russian sentence: its id, a tab, the ids vosk_tts.Synth.g2p_noembed makes, space-separated, a tab,
the text. A sentence vosk-tts fails on (a character its rules keep but the voice does not know, such as "–") is left
out. Needs vosk-tts.
"""
import sys


def main():
    import vosk_tts
    model_dir, sentences, out = sys.argv[1:4]
    synth = vosk_tts.Synth(vosk_tts.Model(model_path=model_dir))
    with open(out, "w", encoding="utf-8") as f:
        for line in open(sentences, encoding="utf-8"):
            key, lang, text = line.rstrip("\n").split("\t")
            if lang != "ru":
                continue
            try:
                ids = synth.g2p_noembed(text)
            except KeyError as e:
                print(f"{key}: vosk-tts fails on {e}", file=sys.stderr)
                continue
            f.write(f"{key}\t{' '.join(map(str, ids))}\t{text}\n")


if __name__ == "__main__":
    main()
