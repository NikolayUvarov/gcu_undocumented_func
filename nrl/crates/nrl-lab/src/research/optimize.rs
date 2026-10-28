use super::*;
use crate::decoder::Decoder;
use evaluate::{Measurement, Timing};
use serde::{Deserialize, Serialize};
use std::time::Instant;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    pub schema: String,
    pub model_id: String,
    pub corpus_id: String,
    pub measurement: Measurement,
    pub calibration_rows: usize,
    pub maximum_absolute_logit_difference: f64,
    pub maximum_absolute_nll_difference: f64,
    pub acceptance: String,
}
impl Study {
    pub fn validate(&self) -> Result<()> {
        self.measurement.validate()?;
        finite(&[
            self.maximum_absolute_logit_difference,
            self.maximum_absolute_nll_difference,
        ])?;
        if self.schema!="nrl.optimization-study/0" || !storage::valid_hash(&self.model_id) || !storage::valid_hash(&self.corpus_id) || !(1..=256).contains(&self.calibration_rows) || !(0.0..=1e-9).contains(&self.maximum_absolute_logit_difference) || !(0.0..=1e-9).contains(&self.maximum_absolute_nll_difference) || self.acceptance!="all split numerics/quality within fixed limits and prepared p95 strictly below reference minimum"{return Err("unsupported optimization study".into());}
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let s: Self = parse(&read(path)?, "nrl.optimization-study/0")?;
        s.validate()?;
        Ok(s)
    }
}
pub(crate) fn paired<F: FnMut(bool) -> Result<()>>(
    spec: &Measurement,
    mut run: F,
) -> Result<(Timing, Timing)> {
    spec.validate()?;
    for i in 0..spec.warmups {
        run(i % 2 == 0)?;
        run(i % 2 != 0)?;
    }
    let (mut reference, mut prepared) = (Vec::new(), Vec::new());
    for i in 0..spec.repeats {
        for flag in [i % 2 == 0, i % 2 != 0] {
            let t = Instant::now();
            run(flag)?;
            let seconds = t.elapsed().as_secs_f64();
            if flag {
                prepared.push(seconds);
            } else {
                reference.push(seconds);
            }
        }
    }
    Ok((evaluate::timing(reference)?, evaluate::timing(prepared)?))
}
fn loss(logits: &[f64], tokens: &[usize], vocab: usize) -> Result<f64> {
    if logits.len() != (tokens.len() - 1) * vocab {
        return Err("optimization logits/targets shape".into());
    }
    let mut sum = 0.0;
    for (row, target) in logits.chunks_exact(vocab).zip(&tokens[1..]) {
        let max = row.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        sum += max + row.iter().map(|v| (v - max).exp()).sum::<f64>().ln() - row[*target];
    }
    finite(&[sum])?;
    Ok(sum)
}
pub fn profile(model: &Decoder, corpus: &Corpus, study: &Study) -> Result<serde_json::Value> {
    study.validate()?;
    if model.model_id() != study.model_id
        || corpus.data_id()? != study.corpus_id
        || !model.patches().is_empty()
    {
        return Err("optimization exact dense model/corpus identity mismatch".into());
    }
    let tokenized = corpus.tokenize(model)?;
    let total_rows: usize = tokenized.iter().map(|t| t.len() - 1).sum();
    let val_rows: usize = corpus
        .documents
        .iter()
        .zip(&tokenized)
        .filter(|(d, _)| d.split == "validation")
        .map(|(_, t)| t.len() - 1)
        .sum();
    let split_rows = |split: &str| {
        corpus
            .documents
            .iter()
            .zip(&tokenized)
            .filter(|(d, _)| d.split == split)
            .map(|(_, t)| t.len() - 1)
            .sum::<usize>()
    };
    let calibration_rows =
        study.calibration_rows.min(split_rows("train")) + study.calibration_rows.min(val_rows);
    let decoder_rows = 2 * total_rows
        + 2 * (study.measurement.warmups + study.measurement.repeats) * val_rows
        + calibration_rows;
    if decoder_rows > study.measurement.max_forward_rows {
        return Err("optimization total decoder work budget exceeded before computation".into());
    }
    let env = evaluate::environment()?;
    let mut quality = Vec::new();
    let (mut maximum, mut quality_delta) = (0.0f64, 0.0f64);
    for split in ["train", "validation", "test"] {
        let (mut a_loss, mut b_loss, mut rows, mut max, mut flips) =
            (0.0, 0.0, 0usize, 0.0f64, 0usize);
        for (_, t) in corpus
            .documents
            .iter()
            .zip(&tokenized)
            .filter(|(d, _)| d.split == split)
        {
            let a = model.forward_reference(&t[..t.len() - 1])?;
            let b = model.forward_prepared(&t[..t.len() - 1])?;
            for (x, y) in a.iter().zip(&b) {
                max = max.max((x - y).abs());
            }
            for (x, y) in a
                .chunks_exact(model.config().vocab)
                .zip(b.chunks_exact(model.config().vocab))
            {
                let best = |v: &[f64]| {
                    v.iter()
                        .enumerate()
                        .max_by(|(i, a), (j, b)| a.total_cmp(b).then(j.cmp(i)))
                        .unwrap()
                        .0
                };
                if best(x) != best(y) {
                    flips += 1;
                }
            }
            a_loss += loss(&a, t, model.config().vocab)?;
            b_loss += loss(&b, t, model.config().vocab)?;
            rows += t.len() - 1;
        }
        let a = a_loss / rows as f64;
        let b = b_loss / rows as f64;
        maximum = maximum.max(max);
        quality_delta = quality_delta.max((a - b).abs());
        quality.push(serde_json::json!({"split":split,"predicted_tokens":rows,"reference_mean_nll":a,"prepared_mean_nll":b,"maximum_absolute_logit_difference":max,"argmax_changes":flips}));
    }
    let validation: Vec<_> = corpus
        .documents
        .iter()
        .zip(&tokenized)
        .filter(|(d, _)| d.split == "validation")
        .map(|(_, t)| t[..t.len() - 1].to_vec())
        .collect();
    let (reference, prepared) = paired(&study.measurement, |flag| {
        for t in &validation {
            std::hint::black_box(if flag {
                model.forward_prepared(t)?
            } else {
                model.forward_reference(t)?
            });
        }
        Ok(())
    })?;
    let cal = Calibration::collect(model, corpus.clone(), 0, study.calibration_rows)?;
    let (x, _) = cal.batch("train")?;
    let dense = model.dense(0)?;
    let block = dense.block()?;
    let mut scratch = vec![0.0; block.scratch_len()];
    let mut y = vec![0.0; dense.dim];
    let (mut component_reference, mut component_prepared) = paired(&study.measurement, |flag| {
        for row in x.chunks_exact(dense.dim) {
            if flag {
                block
                    .forward(row, &mut y, &mut scratch)
                    .map_err(|e| format!("component: {e:?}"))?;
                std::hint::black_box(&y);
            } else {
                std::hint::black_box(dense.forward(row)?);
            }
        }
        Ok(())
    })?;
    let component_scope="isolated original FFN on actual train inputs; reference validates/allocates per row, prepared reuses checked view and scratch; not a whole-model speed claim";
    component_reference.scope = component_scope.into();
    component_prepared.scope = component_scope.into();
    let quality_pass = maximum <= study.maximum_absolute_logit_difference
        && quality_delta <= study.maximum_absolute_nll_difference;
    let timing_pass = prepared.p95_seconds < reference.minimum_seconds;
    Ok(
        serde_json::json!({"schema":"nrl.optimization-report/0","study":study,"study_id":id(study)?,"model_id":model.model_id(),"environment":env,"reference_implementation":"checked FFN view and scratch per token/layer","candidate_implementation":"same operations with immutable borrowed FFN views/scratch once per document; preparation included in full timing","paired_order":"alternate reference/prepared and prepared/reference per repetition","quality":quality,"reference":reference,"prepared":prepared,"component_reference":component_reference,"component_prepared":component_prepared,"decoder_forward_rows":decoder_rows,"calibration_forward_rows":calibration_rows,"component_forward_rows":2*(study.measurement.warmups+study.measurement.repeats)*(x.len()/dense.dim),"quality_pass":quality_pass,"observed_nonoverlapping_timing_pass":timing_pass,"accepted_host_experiment":quality_pass&&timing_pass,"default_path":"per-document prepared after accepted paired host measurement","limits":"same scalar f64 host and current workload only; no native/accelerator/generation/memory/compression guarantee","native_execution":false}),
    )
}
pub fn save_profile(
    path: &Path,
    model: &Decoder,
    corpus: &Corpus,
    study: &Study,
) -> Result<serde_json::Value> {
    let r = profile(model, corpus, study)?;
    save(path, &r)?;
    Ok(r)
}
