//! Fixed paired host generation study, including request preparation and fresh KV caches.
use super::*;
use crate::decoder::Decoder;
use evaluate::Measurement;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    pub schema: String,
    pub model_id: String,
    pub prompts: Vec<Vec<usize>>,
    pub max_new_tokens: usize,
    pub measurement: Measurement,
}
impl Study {
    pub fn validate(&self, model: &Decoder) -> Result<()> {
        self.measurement.validate()?;
        if self.schema != "nrl.generation-study/0"
            || self.model_id != model.model_id()
            || self.prompts.is_empty()
            || self.prompts.len() > 16
            || !(1..=64).contains(&self.max_new_tokens)
            || self.prompts.iter().any(|t| {
                t.is_empty()
                    || t.len()
                        .checked_add(self.max_new_tokens)
                        .is_none_or(|n| n > model.config().context)
                    || t.iter().any(|v| *v >= model.config().vocab)
            })
        {
            return Err("generation study model/prompt/work bound".into());
        }
        let upper = self
            .prompts
            .iter()
            .map(|t| t.len() + self.max_new_tokens)
            .sum::<usize>()
            * (2 + 2 * (self.measurement.warmups + self.measurement.repeats));
        if upper > self.measurement.max_forward_rows {
            return Err("generation total decoder row budget before execution".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        parse(&read(path)?, "nrl.generation-study/0")
    }
}
pub fn profile(model: &Decoder, study: &Study) -> Result<serde_json::Value> {
    study.validate(model)?;
    let mut quality = Vec::new();
    let mut identical = true;
    let mut generated = 0;
    for prompt in &study.prompts {
        let reference = model.generate_reference(prompt, study.max_new_tokens)?;
        let prepared = model.generate_prepared(prompt, study.max_new_tokens)?;
        identical &= reference == prepared;
        generated += prepared.len();
        quality.push(serde_json::json!({"prompt":prompt,"reference":reference,"prepared":prepared,"identical":reference==prepared}));
    }
    let (mut reference, mut prepared) = optimize::paired(&study.measurement, |flag| {
        for t in &study.prompts {
            std::hint::black_box(if flag {
                model.generate_prepared(t, study.max_new_tokens)?
            } else {
                model.generate_reference(t, study.max_new_tokens)?
            });
        }
        Ok(())
    })?;
    let scope="fresh request including checked FFN preparation, prompt prefill, greedy decoding and cache allocation; no load/tokenization; early EOS retained";
    reference.scope = scope.into();
    prepared.scope = scope.into();
    let accepted = identical && prepared.p95_seconds < reference.minimum_seconds;
    let upper = self_upper(study);
    Ok(
        serde_json::json!({"schema":"nrl.generation-report/0","study":study,"study_id":id(study)?,"environment":evaluate::environment()?,"quality":quality,"identical_tokens":identical,"generated_tokens_per_pass":generated,"reference":reference,"prepared":prepared,"reference_generated_tokens_per_second":generated as f64/reference.median_seconds,"prepared_generated_tokens_per_second":generated as f64/prepared.median_seconds,"decoder_forward_rows_upper_bound":upper,"accepted_host_experiment":accepted,"acceptance":"identical generated IDs and prepared p95 strictly below reference minimum","step_implementation":"prepared checked FFNs and scratch; private append-only KV tail rolled back on refusal, no full prefix clone; public Decoder::step unchanged","limits":"this model/prompts/executable/host only; includes prefill, not isolated steady-state decode throughput; no native/parallel/GPU claim","native_execution":false}),
    )
}
fn self_upper(s: &Study) -> usize {
    s.prompts
        .iter()
        .map(|t| t.len() + s.max_new_tokens)
        .sum::<usize>()
        * (2 + 2 * (s.measurement.warmups + s.measurement.repeats))
}
pub fn save_profile(path: &Path, model: &Decoder, study: &Study) -> Result<serde_json::Value> {
    let r = profile(model, study)?;
    save(path, &r)?;
    Ok(r)
}
