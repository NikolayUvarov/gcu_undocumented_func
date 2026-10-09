#!/bin/bash
# fetch_all.sh <work>: every synthesis model compared, each into <work>/dl/<name> (about 9 GB); PYTHON has huggingface_hub.
set -e
W=$1; DL=$W/dl; G="${PYTHON:-python3} -I $(dirname "$0")/hf_get.py"
for v in ru_RU-irina-medium ru_RU-dmitri-medium ru_RU-denis-medium ru_RU-ruslan-medium en_US-lessac-high en_US-ryan-high; do
  $G $DL/piper-$v csukuangfj/vits-piper-$v
done
$G $DL/kokoro csukuangfj/kokoro-multi-lang-v1_0
$G $DL/kitten-nano csukuangfj/kitten-nano-en-v0_2-fp16
$G $DL/kitten-mini csukuangfj/kitten-mini-en-v0_1-fp16
for v in 0.7 0.9; do
  mkdir -p $DL/vosk-tts-$v
  curl -fL -o $DL/vosk-tts-$v/model.zip https://alphacephei.com/vosk/models/vosk-model-tts-ru-$v-multi.zip
  (cd $DL/vosk-tts-$v && unzip -q -o model.zip && rm model.zip)
done
$G $DL/espeech ESpeech/ESpeech-TTS-1_RL-V2
$G $DL/qwen3-base Qwen/Qwen3-TTS-12Hz-0.6B-Base
$G $DL/chatterbox ResembleAI/chatterbox ve.pt t3_mtl23ls_v2.safetensors s3gen.pt grapheme_mtl_merged_expanded_v1.json conds.pt Cangjie5_TC.json
# Pocket TTS, the ESpeech vocoder, RUAccent and UTMOSv2 fetch their own files into HF_HOME on first use.
