#!/bin/bash
# fetch_all.sh <work>: the FLEURS test sets and every model compared, each into <work>/dl/<name> (about 4 GB).
W=$1; F=$(dirname "$0")/fetch.sh; DL=$W/dl; HF=https://huggingface.co
r() { echo "$HF/$1/resolve/main/$2"; }
$F $DL/fleurs-ru $(r datasets/google/fleurs data/ru_ru/test.tsv) $(r datasets/google/fleurs data/ru_ru/audio/test.tar.gz)
$F $DL/fleurs-en $(r datasets/google/fleurs data/en_us/test.tsv) $(r datasets/google/fleurs data/en_us/audio/test.tar.gz)
$F $DL/vosk-kaldi-small-ru https://alphacephei.com/vosk/models/vosk-model-small-ru-0.22.zip
$F $DL/vosk-kaldi-small-en https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip
m=alphacep/vosk-model-small-ru; $F $DL/vosk-zf-small-ru $(r $m am/encoder.int8.onnx) $(r $m am/decoder.int8.onnx) $(r $m am/joiner.int8.onnx) $(r $m lang/tokens.txt)
$F $DL/vosk-zf-small-ru-fp32 $(r $m am/encoder.onnx) $(r $m am/decoder.onnx) $(r $m am/joiner.onnx) $(r $m lang/tokens.txt)
m=alphacep/vosk-model-small-streaming-ru; $F $DL/vosk-zf-small-streaming-ru $(r $m am-onnx/encoder.int8.onnx) $(r $m am-onnx/decoder.int8.onnx) $(r $m am-onnx/joiner.int8.onnx) $(r $m lang/tokens.txt)
m=alphacep/vosk-model-ru; $F $DL/vosk-zf-ru $(r $m am-onnx/encoder.int8.onnx) $(r $m am-onnx/decoder.int8.onnx) $(r $m am-onnx/joiner.int8.onnx) $(r $m lang/tokens.txt)
m=alphacep/vosk-model-streaming-ru; $F $DL/vosk-zf-streaming-ru $(r $m am-onnx/encoder.int8.onnx) $(r $m am-onnx/decoder.int8.onnx) $(r $m am-onnx/joiner.int8.onnx) $(r $m lang/tokens.txt)
$F $DL/vosk-zf-streaming-ru-fp32 $(r $m am-onnx/encoder.onnx) $(r $m am-onnx/decoder.onnx) $(r $m am-onnx/joiner.onnx) $(r $m lang/tokens.txt)
m=csukuangfj/sherpa-onnx-streaming-t-one-russian-2025-09-08; $F $DL/t-one $(r $m model.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-whisper-tiny; $F $DL/whisper-tiny $(r $m tiny-encoder.int8.onnx) $(r $m tiny-decoder.int8.onnx) $(r $m tiny-tokens.txt)
$F $DL/whisper-tiny-fp32 $(r $m tiny-encoder.onnx) $(r $m tiny-decoder.onnx) $(r $m tiny-tokens.txt)
m=csukuangfj/sherpa-onnx-whisper-base; $F $DL/whisper-base $(r $m base-encoder.int8.onnx) $(r $m base-decoder.int8.onnx) $(r $m base-tokens.txt)
$F $DL/whisper-base-fp32 $(r $m base-encoder.onnx) $(r $m base-decoder.onnx) $(r $m base-tokens.txt)
m=csukuangfj/sherpa-onnx-moonshine-tiny-en-int8; $F $DL/moonshine-tiny $(r $m preprocess.onnx) $(r $m encode.int8.onnx) $(r $m uncached_decode.int8.onnx) $(r $m cached_decode.int8.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-moonshine-base-en-int8; $F $DL/moonshine-base $(r $m preprocess.onnx) $(r $m encode.int8.onnx) $(r $m uncached_decode.int8.onnx) $(r $m cached_decode.int8.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-streaming-zipformer-en-20M-2023-02-17; $F $DL/zf-en-20m $(r $m encoder-epoch-99-avg-1.int8.onnx) $(r $m decoder-epoch-99-avg-1.int8.onnx) $(r $m joiner-epoch-99-avg-1.int8.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-zipformer-en-libriheavy-20230926-small; $F $DL/zf-en-libriheavy-small $(r $m encoder-epoch-90-avg-20.int8.onnx) $(r $m decoder-epoch-90-avg-20.int8.onnx) $(r $m joiner-epoch-90-avg-20.int8.onnx) $(r $m tokens.txt)
m=k2-fsa/sherpa-onnx-zipformer-gigaspeech-2023-12-12; $F $DL/zf-en-gigaspeech $(r $m encoder-epoch-30-avg-1.onnx) $(r $m decoder-epoch-30-avg-1.onnx) $(r $m joiner-epoch-30-avg-1.onnx) $(r $m encoder-epoch-30-avg-1.int8.onnx) $(r $m decoder-epoch-30-avg-1.int8.onnx) $(r $m joiner-epoch-30-avg-1.int8.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-streaming-zipformer-en-kroko-2025-08-06; $F $DL/kroko-en $(r $m encoder.onnx) $(r $m decoder.onnx) $(r $m joiner.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000; $F $DL/parakeet-110m $(r $m model.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-nemo-ctc-en-conformer-small; $F $DL/nemo-en-conformer-small $(r $m model.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-nemo-ctc-en-conformer-medium; $F $DL/nemo-en-conformer-medium $(r $m model.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-nemo-fast-conformer-ctc-be-de-en-es-fr-hr-it-pl-ru-uk-20k; $F $DL/nemo-ml-fc $(r $m model.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8; $F $DL/parakeet-0.6b-v3 $(r $m encoder.int8.onnx) $(r $m decoder.int8.onnx) $(r $m joiner.int8.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-nemo-ctc-giga-am-v3-russian-2025-12-16; $F $DL/gigaam-v3-ctc $(r $m model.int8.onnx) $(r $m tokens.txt)
m=csukuangfj/sherpa-onnx-nemo-transducer-giga-am-v3-russian-2025-12-16; $F $DL/gigaam-v3-rnnt $(r $m encoder.int8.onnx) $(r $m decoder.onnx) $(r $m joiner.onnx) $(r $m tokens.txt)
for d in vosk-kaldi-small-ru vosk-kaldi-small-en; do (cd $DL/$d && unzip -q -o ./*.zip); done
