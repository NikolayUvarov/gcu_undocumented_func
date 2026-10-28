use super::*;
use crate::{decoder::Decoder, model::Source};
use evaluate::{Measurement, Report};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub layer: usize,
    pub transform: Transform,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub id: String,
    pub entries: Vec<Entry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recovery {
    pub start_plan: String,
    pub rounds: usize,
    pub learning_rate: f64,
    pub row_limit: usize,
    pub protocol: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    pub schema: String,
    pub baseline_model_id: String,
    pub corpus_id: String,
    pub row_limit: usize,
    pub local_steps: usize,
    pub local_learning_rate: f64,
    pub maximum_validation_nll_increase: f64,
    pub maximum_package_ratio: f64,
    pub measurement: Measurement,
    pub plans: Vec<Plan>,
    pub recovery: Recovery,
}
impl Study {
    pub fn validate(&self) -> Result<()> {
        finite(&[
            self.local_learning_rate,
            self.maximum_validation_nll_increase,
            self.maximum_package_ratio,
            self.recovery.learning_rate,
        ])?;
        self.measurement.validate()?;
        if self.schema != "nrl.study-profile/0"
            || !storage::valid_hash(&self.baseline_model_id)
            || !storage::valid_hash(&self.corpus_id)
            || !(1..=256).contains(&self.row_limit)
            || !(1..=64).contains(&self.local_steps)
            || self.local_learning_rate <= 0.0
            || self.local_learning_rate > 1.0
            || !(0.0..=1.0).contains(&self.maximum_validation_nll_increase)
            || !(1.0..=4.0).contains(&self.maximum_package_ratio)
            || self.plans.is_empty()
            || self.plans.len() > 8
            || !(1..=8).contains(&self.recovery.rounds)
            || !(1..=256).contains(&self.recovery.row_limit)
            || self.recovery.learning_rate <= 0.0
            || self.recovery.learning_rate > 1.0
            || self.recovery.protocol != "frozen-teacher-on-student-input/0"
        {
            return Err("unsupported/bounded study profile".into());
        }
        let mut ids = BTreeSet::new();
        for p in &self.plans {
            if !name(&p.id)
                || p.id == "dense"
                || !ids.insert(p.id.clone())
                || p.entries.is_empty()
                || p.entries.len() > 3
            {
                return Err("invalid/duplicate plan ID or entry count".into());
            }
            let mut layers = BTreeSet::new();
            for e in &p.entries {
                e.transform.validate()?;
                if e.layer >= 12 || !layers.insert(e.layer) {
                    return Err("duplicate/out-of-profile plan layer".into());
                }
            }
        }
        if !ids.contains(&self.recovery.start_plan) {
            return Err("unknown recovery start plan".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let s: Self = parse(&read(path)?, "nrl.study-profile/0")?;
        s.validate()?;
        Ok(s)
    }
    pub fn study_id(&self) -> Result<String> {
        self.validate()?;
        id(self)
    }
    pub fn verify(&self, teacher: &Decoder, corpus: &Corpus) -> Result<()> {
        self.validate()?;
        if teacher.model_id() != self.baseline_model_id
            || !teacher.patches().is_empty()
            || corpus.data_id()? != self.corpus_id
        {
            return Err("study exact teacher/corpus identity mismatch".into());
        }
        corpus.tokenize(teacher)?;
        for p in &self.plans {
            for e in &p.entries {
                let src = teacher.dense(e.layer)?;
                if e.transform.width > src.hidden {
                    return Err("plan width exceeds source".into());
                }
            }
        }
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<&Plan> {
        self.plans
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "unknown plan ID".into())
    }
}
pub struct Applied {
    pub model: Decoder,
    pub plan: Plan,
    pub study_id: String,
    pub states: Vec<LocalState>,
}
impl Applied {
    pub fn training_report(&self) -> Result<serde_json::Value> {
        Ok(
            serde_json::json!({"schema":"nrl.applied-plan/0","plan":self.plan,"study_id":self.study_id,"model_id":self.model.model_id(),"local_runs":self.states.iter().map(LocalState::report).collect::<Result<Vec<_>>>()?,"composition":"all selected original-teacher local blocks installed together; whole-model validation required","native_execution":false}),
        )
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        fs::create_dir(path).map_err(|e| e.to_string())?;
        self.model.save(&path.join("model"))?;
        for s in &self.states {
            s.save(&path.join(format!("layer-{}.json", s.calibration.layer)))?;
        }
        save(&path.join("complete.json"), &self.training_report()?)
    }
}
pub fn apply(teacher: &Decoder, corpus: &Corpus, study: &Study, plan_id: &str) -> Result<Applied> {
    study.verify(teacher, corpus)?;
    let plan = study.get(plan_id)?.clone();
    let mut patches = BTreeMap::new();
    let mut states = Vec::new();
    let mut sources = vec![corpus.source.clone()];
    for e in &plan.entries {
        let cal = Calibration::collect(teacher, corpus.clone(), e.layer, study.row_limit)?;
        let output_source=Source { identifier:format!("derived://nrl-plan/{}/layer-{}",plan.id,e.layer),sha256:None,licence:corpus.source.licence.clone(),notices:vec!["Rust train-only initializer and full-batch local SGD; validation selects; source weights/tokenizer notices retained.".into()]};
        let config = LocalConfig {
            schema: "nrl.local-config/0".into(),
            transform: e.transform.clone(),
            max_steps: study.local_steps,
            learning_rate: study.local_learning_rate,
            output_source,
        };
        let mut state = LocalState::start(teacher, cal, config)?;
        state.advance(study.local_steps)?;
        patches.insert(e.layer, state.best.clone());
        sources.push(state.source_record()?);
        states.push(state);
    }
    Ok(Applied {
        model: teacher.with_patches(patches, &sources)?,
        plan,
        study_id: study.study_id()?,
        states,
    })
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub plan_id: String,
    pub validation: Option<Report>,
    pub refusal: Option<String>,
    pub training: Option<serde_json::Value>,
}
fn package(r: &Report) -> Result<f64> {
    r.model_information["package_bytes"]
        .as_u64()
        .map(|n| n as f64)
        .filter(|n| *n > 0.0)
        .ok_or_else(|| "missing actual package bytes".into())
}
fn cost(r: &Report) -> Result<(f64, f64, f64)> {
    Ok((
        r.quality.value,
        r.timing
            .as_ref()
            .ok_or("missing candidate timing")?
            .median_seconds,
        package(r)?,
    ))
}
pub fn select(
    baseline: &Report,
    candidates: &[Candidate],
    study: &Study,
) -> Result<serde_json::Value> {
    study.validate()?;
    baseline.validate()?;
    if baseline.profile.split != "validation"
        || baseline.profile.baseline_id != study.baseline_model_id
        || baseline.model_id != study.baseline_model_id
        || baseline.profile.data_id != study.corpus_id
        || baseline.profile.measurement.as_ref() != Some(&study.measurement)
        || baseline.profile.task != "language-next-token/0"
    {
        return Err("search requires exact measured validation baseline".into());
    }
    if candidates.len() != study.plans.len() {
        return Err("search candidate count differs from frozen study".into());
    }
    let mut assessments = Vec::new();
    let mut accepted = Vec::new();
    let mut measured = vec![("dense".to_string(), cost(baseline)?)];
    for (c, p) in candidates.iter().zip(&study.plans) {
        if c.plan_id != p.id {
            return Err("candidate order/identity differs from study".into());
        }
        if let Some(r) = &c.validation {
            if c.refusal.is_some() {
                return Err("candidate both measured and refused".into());
            }
            evaluate::compare(baseline, r)?;
            let quality =
                r.quality.value - baseline.quality.value <= study.maximum_validation_nll_increase;
            let disk = package(r)? / package(baseline)? <= study.maximum_package_ratio;
            let speed = r
                .timing
                .as_ref()
                .ok_or("missing candidate timing")?
                .p95_seconds
                < baseline
                    .timing
                    .as_ref()
                    .ok_or("missing baseline timing")?
                    .minimum_seconds;
            let eligible = quality && disk && speed;
            let costs = cost(r)?;
            measured.push((c.plan_id.clone(), costs));
            assessments.push(serde_json::json!({"plan_id":c.plan_id,"quality_budget_pass":quality,"actual_package_budget_pass":disk,"observed_nonoverlapping_timing_pass":speed,"eligible":eligible}));
            if eligible {
                accepted.push((c.plan_id.clone(), costs));
            }
        } else {
            if c.refusal.as_ref().is_none_or(|r| r.is_empty()) {
                return Err("candidate has neither result nor refusal".into());
            }
            assessments.push(
                serde_json::json!({"plan_id":c.plan_id,"eligible":false,"refusal":c.refusal}),
            );
        }
    }
    accepted.sort_by(|a, b| {
        a.1 .1
            .total_cmp(&b.1 .1)
            .then(a.1 .0.total_cmp(&b.1 .0))
            .then(a.0.cmp(&b.0))
    });
    let pareto: Vec<_> = measured
        .iter()
        .filter(|(_, a)| {
            !measured.iter().any(|(_, b)| {
                b.0 <= a.0 && b.1 <= a.1 && b.2 <= a.2 && (b.0 < a.0 || b.1 < a.1 || b.2 < a.2)
            })
        })
        .map(|(name, _)| name.clone())
        .collect();
    Ok(
        serde_json::json!({"schema":"nrl.plan-selection/0","study_id":study.study_id()?,"selected_plan":accepted.first().map_or("dense",|a|a.0.as_str()),"assessments":assessments,"pareto":pareto,"selection_split":"validation","test_used_for_selection":false,"rule":"quality delta and actual package budgets; candidate p95 strictly below dense minimum; among eligible lowest median then NLL then plan ID; otherwise dense","timing_scope":"observed scalar host run only; no statistical or general performance guarantee","process_rss_used_for_selection":false}),
    )
}
pub struct Search {
    pub study: Study,
    pub corpus: Corpus,
    pub baseline: Report,
    pub candidates: Vec<Candidate>,
    pub models: BTreeMap<String, Decoder>,
    pub selection: serde_json::Value,
    pub selected: Decoder,
    pub selected_test: Report,
}
pub fn search(teacher: &Decoder, corpus: Corpus, study: Study) -> Result<Search> {
    study.verify(teacher, &corpus)?;
    let baseline = evaluate::evaluate_language(
        teacher,
        teacher,
        &corpus,
        "validation",
        Some(study.measurement.clone()),
    )?;
    let mut candidates = Vec::new();
    let mut models = BTreeMap::new();
    for p in &study.plans {
        let candidate = (|| -> Result<(Applied, Report)> {
            let a = apply(teacher, &corpus, &study, &p.id)?;
            let r = evaluate::evaluate_language(
                teacher,
                &a.model,
                &corpus,
                "validation",
                Some(study.measurement.clone()),
            )?;
            Ok((a, r))
        })();
        match candidate {
            Ok((a, r)) => {
                let training = Some(a.training_report()?);
                models.insert(p.id.clone(), a.model);
                candidates.push(Candidate {
                    plan_id: p.id.clone(),
                    validation: Some(r),
                    refusal: None,
                    training,
                });
            }
            Err(e) => candidates.push(Candidate {
                plan_id: p.id.clone(),
                validation: None,
                refusal: Some(e),
                training: None,
            }),
        }
    }
    let selection = select(&baseline, &candidates, &study)?;
    let chosen = selection["selected_plan"]
        .as_str()
        .ok_or("missing selection")?;
    let selected = if chosen == "dense" {
        teacher.clone()
    } else {
        models.get(chosen).ok_or("selected model missing")?.clone()
    };
    let selected_test = evaluate::evaluate_language(teacher, &selected, &corpus, "test", None)?;
    Ok(Search {
        study,
        corpus,
        baseline,
        candidates,
        models,
        selection,
        selected,
        selected_test,
    })
}
impl Search {
    pub fn save(&self, path: &Path) -> Result<()> {
        fs::create_dir(path).map_err(|e| e.to_string())?;
        save(&path.join("study.json"), &self.study)?;
        save(&path.join("corpus.json"), &self.corpus)?;
        self.baseline.save(&path.join("baseline-validation.json"))?;
        for c in &self.candidates {
            if let Some(r) = &c.validation {
                r.save(&path.join(format!("{}-validation.json", c.plan_id)))?;
            }
            if let Some(m) = self.models.get(&c.plan_id) {
                m.save(&path.join(format!("{}-model", c.plan_id)))?;
            }
        }
        self.selected.save(&path.join("selected-model"))?;
        self.selected_test.save(&path.join("selected-test.json"))?;
        save(
            &path.join("complete.json"),
            &serde_json::json!({"schema":"nrl.search/0","study_id":self.study.study_id()?,"baseline_report_id":self.baseline.report_id()?,"candidates":self.candidates,"selection":self.selection,"selected_model_id":self.selected.model_id(),"selected_test_report_id":self.selected_test.report_id()?,"native_execution":false}),
        )
    }
}
