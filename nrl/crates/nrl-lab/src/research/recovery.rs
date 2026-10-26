use super::*;
use crate::{decoder::Decoder, model::Source};
use data::PairDocument;
use evaluate::Report;
use plan::Study;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs};
const STUDENT: &str = "frozen-teacher-on-student-input/0";
const CONTROL: &str = "frozen-teacher-on-original-input/0";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pairs {
    pub schema: String,
    pub protocol: String,
    pub teacher_model_id: String,
    pub student_model_id: String,
    pub input_model_id: String,
    pub corpus_id: String,
    pub split: String,
    pub dim: usize,
    pub row_limit: usize,
    pub layers: BTreeMap<usize, Vec<PairDocument>>,
}
impl Pairs {
    pub fn validate(&self) -> Result<()> {
        if self.schema != "nrl.recovery-pairs/0"
            || !matches!(self.protocol.as_str(), STUDENT | CONTROL)
            || self.split != "train"
            || self.dim == 0
            || self.dim > 128
            || !(1..=256).contains(&self.row_limit)
            || !(2..=3).contains(&self.layers.len())
            || [
                &self.teacher_model_id,
                &self.student_model_id,
                &self.input_model_id,
                &self.corpus_id,
            ]
            .iter()
            .any(|s| !storage::valid_hash(s))
            || self.input_model_id
                != if self.protocol == STUDENT {
                    self.student_model_id.clone()
                } else {
                    self.teacher_model_id.clone()
                }
        {
            return Err("invalid recovery pair profile/identities/budget".into());
        }
        let first = self
            .layers
            .values()
            .next()
            .ok_or("empty recovery pair layers")?;
        for (layer, docs) in &self.layers {
            if *layer >= 12 || docs.is_empty() || docs.len() > 64 || docs.len() != first.len() {
                return Err("invalid recovery layer/document profile".into());
            }
            let mut count = 0;
            for (d, reference) in docs.iter().zip(first) {
                if !name(&d.document_id)
                    || !name(&d.group)
                    || d.tokens.is_empty()
                    || d.tokens.len() > self.row_limit
                    || d.tokens.iter().any(|t| *t >= 1024)
                    || d.input.len() != d.tokens.len() * self.dim
                    || d.target.len() != d.input.len()
                    || d.document_id != reference.document_id
                    || d.group != reference.group
                    || d.tokens != reference.tokens
                {
                    return Err("recovery pair ordering/token/shape mismatch".into());
                }
                finite(&d.input)?;
                finite(&d.target)?;
                count += d.tokens.len();
            }
            if count == 0 || count > self.row_limit {
                return Err("recovery pair row limit".into());
            }
        }
        Ok(())
    }
    pub fn pair_id(&self) -> Result<String> {
        self.validate()?;
        id(self)
    }
    pub fn verify(&self, teacher: &Decoder, student: &Decoder, corpus: &Corpus) -> Result<()> {
        self.validate()?;
        let expected = collect(teacher, student, corpus, self.row_limit, &self.protocol)?;
        if self.pair_id()? != expected.pair_id()? {
            return Err(
                "recovery pairs differ from recomputed actual trajectory/teacher target".into(),
            );
        }
        Ok(())
    }
    fn batch(&self, layer: usize) -> Result<(Vec<f64>, Vec<f64>)> {
        self.validate()?;
        let docs = self.layers.get(&layer).ok_or("missing recovery layer")?;
        Ok((
            docs.iter().flat_map(|d| d.input.clone()).collect(),
            docs.iter().flat_map(|d| d.target.clone()).collect(),
        ))
    }
}
pub fn collect(
    teacher: &Decoder,
    student: &Decoder,
    corpus: &Corpus,
    row_limit: usize,
    protocol: &str,
) -> Result<Pairs> {
    if !matches!(protocol, STUDENT | CONTROL)
        || !(1..=256).contains(&row_limit)
        || !teacher.patches().is_empty()
        || teacher.base_content_id()? != student.base_content_id()?
        || !(2..=3).contains(&student.patches().len())
    {
        return Err("unsupported recovery source/protocol/patch-count/row budget".into());
    }
    let tokens = corpus.tokenize(teacher)?;
    let layers: BTreeMap<_, _> = student.patches().keys().map(|k| (*k, Vec::new())).collect();
    let input_model = if protocol == STUDENT {
        student
    } else {
        teacher
    };
    let mut pairs = Pairs {
        schema: "nrl.recovery-pairs/0".into(),
        protocol: protocol.into(),
        teacher_model_id: teacher.model_id().into(),
        student_model_id: student.model_id().into(),
        input_model_id: input_model.model_id().into(),
        corpus_id: corpus.data_id()?,
        split: "train".into(),
        dim: teacher.config().dim,
        row_limit,
        layers,
    };
    let mut remaining = row_limit;
    for (d, tokens) in corpus
        .documents
        .iter()
        .zip(tokens)
        .filter(|(d, _)| d.split == "train")
    {
        if remaining == 0 {
            break;
        }
        let count = (tokens.len() - 1).min(remaining);
        let prefix = &tokens[..count];
        let trace = input_model.trace(prefix)?;
        for (layer, docs) in &mut pairs.layers {
            let input = trace.layers[*layer].input.clone();
            let target = teacher.dense(*layer)?.forward(&input)?;
            docs.push(PairDocument {
                document_id: d.id.clone(),
                group: d.group.clone(),
                tokens: prefix.to_vec(),
                input,
                target,
            });
        }
        remaining -= count;
    }
    pairs.validate()?;
    Ok(pairs)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerFit {
    pub layer: usize,
    pub before: local::LocalMetrics,
    pub after: local::LocalMetrics,
}
pub fn update(student: &Decoder, pairs: &Pairs, rate: f64) -> Result<(Decoder, Vec<LayerFit>)> {
    pairs.validate()?;
    if pairs.student_model_id != student.model_id()
        || !rate.is_finite()
        || rate <= 0.0
        || rate > 1.0
        || pairs.layers.keys().copied().collect::<Vec<_>>()
            != student.patches().keys().copied().collect::<Vec<_>>()
    {
        return Err("recovery update source/layers/rate mismatch".into());
    }
    let mut patches = student.patches();
    let mut fits = Vec::new();
    for (layer, artifact) in student.patches() {
        let (x, y) = pairs.batch(layer)?;
        let before = local::metrics(&artifact, &x, &y)?;
        let e = crate::evaluate(&artifact, &x, &y)?;
        let next = artifact.step(&e.gradients, rate)?;
        let after = local::metrics(&next, &x, &y)?;
        patches.insert(layer, next);
        fits.push(LayerFit {
            layer,
            before,
            after,
        });
    }
    Ok((student.with_patches(patches, &[])?, fits))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Difference {
    pub output_values: usize,
    pub logit_mse: f64,
    pub maximum_absolute_logit_error: f64,
}
pub fn difference(
    teacher: &Decoder,
    student: &Decoder,
    corpus: &Corpus,
    split: &str,
) -> Result<Difference> {
    if teacher.base_content_id()? != student.base_content_id()? {
        return Err("incompatible logit difference baseline".into());
    }
    let tokens = corpus.tokenize(teacher)?;
    let mut count = 0;
    let mut sum = 0.0;
    let mut maximum: f64 = 0.0;
    for (_, t) in corpus
        .documents
        .iter()
        .zip(tokens)
        .filter(|(d, _)| d.split == split)
    {
        let a = teacher.forward(&t[..t.len() - 1])?;
        let b = student.forward(&t[..t.len() - 1])?;
        for (x, y) in a.iter().zip(b) {
            let e = (x - y).abs();
            sum += e * e;
            maximum = maximum.max(e);
            count += 1;
        }
    }
    if count == 0 {
        return Err("empty logit difference split".into());
    }
    let mse = sum / count as f64;
    finite(&[mse, maximum])?;
    Ok(Difference {
        output_values: count,
        logit_mse: mse,
        maximum_absolute_logit_error: maximum,
    })
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Round {
    pub round: usize,
    pub student_model_id: String,
    pub pair_id: Option<String>,
    pub validation_nll: f64,
    pub accumulated_error: Difference,
    pub fits: Vec<LayerFit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    pub protocol: String,
    pub selected_round: usize,
    pub selected_raw_model_id: String,
    pub history: Vec<Round>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema: String,
    pub teacher_model_id: String,
    pub initial_model_id: String,
    pub study: Study,
    pub corpus: Corpus,
    pub optimizer: String,
    pub update_order: String,
    pub arms: Vec<Arm>,
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        self.study.validate()?;
        self.corpus.validate()?;
        if self.schema != "nrl.recovery-record/0"
            || self.teacher_model_id != self.study.baseline_model_id
            || self.corpus.data_id()? != self.study.corpus_id
            || !storage::valid_hash(&self.initial_model_id)
            || self.optimizer != "full-batch-local-sgd/0"
            || self.update_order
                != "all layers use the same frozen pre-round trajectory; apply together; no RNG"
            || self.arms.len() != 2
        {
            return Err("invalid recovery record profile".into());
        }
        for (arm, protocol) in self.arms.iter().zip([STUDENT, CONTROL]) {
            if arm.protocol != protocol
                || arm.history.len() != self.study.recovery.rounds + 1
                || arm.selected_round >= arm.history.len()
            {
                return Err("recovery arm protocol/round bounds".into());
            }
            let mut best = 0;
            for (i, r) in arm.history.iter().enumerate() {
                finite(&[
                    r.validation_nll,
                    r.accumulated_error.logit_mse,
                    r.accumulated_error.maximum_absolute_logit_error,
                ])?;
                if r.round != i
                    || r.validation_nll < 0.0
                    || r.accumulated_error.logit_mse < 0.0
                    || r.accumulated_error.maximum_absolute_logit_error < 0.0
                    || r.accumulated_error.output_values == 0
                    || !storage::valid_hash(&r.student_model_id)
                    || (i == 0
                        && (r.student_model_id != self.initial_model_id
                            || r.pair_id.is_some()
                            || !r.fits.is_empty()))
                    || (i > 0
                        && (r.pair_id.as_ref().is_none_or(|s| !storage::valid_hash(s))
                            || r.fits.len()
                                != self
                                    .study
                                    .get(&self.study.recovery.start_plan)?
                                    .entries
                                    .len()))
                {
                    return Err("invalid recovery history identity/metric/shape".into());
                }
                for f in &r.fits {
                    for m in [&f.before, &f.after] {
                        finite(&[
                            m.mse,
                            m.nmse,
                            m.mae,
                            m.max_absolute_error,
                            m.target_mean_energy,
                        ])?;
                        if m.mse < 0.0
                            || m.nmse < 0.0
                            || m.mae < 0.0
                            || m.values == 0
                            || m.values > 256 * 128
                            || m.max_absolute_error < 0.0
                            || m.target_mean_energy < 0.0
                            || m.near_zero_energy != (m.target_mean_energy <= 1e-12)
                            || m.nmse != m.mse / m.target_mean_energy.max(1e-12)
                            || m.outside_grid_fraction
                                .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
                        {
                            return Err("invalid local recovery metric".into());
                        }
                    }
                }
                if r.validation_nll < arm.history[best].validation_nll {
                    best = i;
                }
            }
            if arm.selected_round != best
                || arm.selected_raw_model_id != arm.history[best].student_model_id
            {
                return Err(
                    "recovery selection differs from earliest best validation round".into(),
                );
            }
        }
        Ok(())
    }
    pub fn record_id(&self) -> Result<String> {
        self.validate()?;
        id(self)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        save(
            path,
            &serde_json::json!({"schema":"nrl.recovery-envelope/0","payload_sha256":self.record_id()?,"payload":self}),
        )
    }
    pub fn load(path: &Path) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Envelope {
            schema: String,
            payload_sha256: String,
            payload: Record,
        }
        let e: Envelope = parse(&read(path)?, "nrl.recovery-envelope/0")?;
        if e.schema != "nrl.recovery-envelope/0" || e.payload_sha256 != e.payload.record_id()? {
            return Err("recovery record payload hash mismatch".into());
        }
        Ok(e.payload)
    }
}
fn arm(
    teacher: &Decoder,
    initial: &Decoder,
    corpus: &Corpus,
    study: &Study,
    protocol: &str,
) -> Result<(Arm, Decoder, Vec<Pairs>)> {
    let mut current = initial.clone();
    let mut best = initial.clone();
    let mut selected = 0;
    let mut history: Vec<Round> = Vec::new();
    let mut saved_pairs = Vec::new();
    let mut fits = Vec::new();
    let mut pair_id = None;
    for round in 0..=study.recovery.rounds {
        let (quality, _, _) = evaluate::language_quality(&current, corpus, "validation")?;
        let accumulated_error = difference(teacher, &current, corpus, "validation")?;
        if history.is_empty() || quality.value < history[selected].validation_nll {
            best = current.clone();
            selected = round;
        }
        history.push(Round {
            round,
            student_model_id: current.model_id().into(),
            pair_id: pair_id.take(),
            validation_nll: quality.value,
            accumulated_error,
            fits: std::mem::take(&mut fits),
        });
        if round == study.recovery.rounds {
            break;
        }
        let pairs = collect(
            teacher,
            &current,
            corpus,
            study.recovery.row_limit,
            protocol,
        )?;
        pair_id = Some(pairs.pair_id()?);
        let (next, measurements) = update(&current, &pairs, study.recovery.learning_rate)?;
        current = next;
        fits = measurements;
        saved_pairs.push(pairs);
    }
    Ok((
        Arm {
            protocol: protocol.into(),
            selected_round: selected,
            selected_raw_model_id: best.model_id().into(),
            history,
        },
        best,
        saved_pairs,
    ))
}
fn recommend(baseline: &Report, student: &Report, study: &Study) -> Result<serde_json::Value> {
    let b = baseline;
    let s = student;
    evaluate::compare(b, s)?;
    let quality_pass = s.quality.value - b.quality.value <= study.maximum_validation_nll_increase;
    let package_pass = s.model_information["package_bytes"]
        .as_u64()
        .ok_or("missing student package bytes")? as f64
        / b.model_information["package_bytes"]
            .as_u64()
            .ok_or("missing baseline package bytes")? as f64
        <= study.maximum_package_ratio;
    let time_pass = s
        .timing
        .as_ref()
        .ok_or("missing recovery timing")?
        .p95_seconds
        < b.timing
            .as_ref()
            .ok_or("missing baseline timing")?
            .minimum_seconds;
    let recommendation = serde_json::json!({"recommended":"dense","quality_budget_pass":quality_pass,"actual_package_budget_pass":package_pass,"observed_nonoverlapping_timing_pass":time_pass,"selection_split":"validation","test_used_for_selection":false,"rule":"student only if all three predeclared quality/package/nonoverlap timing conditions pass, otherwise dense","native_execution":false});
    let mut recommendation = recommendation;
    if quality_pass && package_pass && time_pass {
        recommendation["recommended"] = serde_json::json!("student");
    }
    Ok(recommendation)
}
pub struct Run {
    pub record: Record,
    pub initial: Decoder,
    pub student: Decoder,
    pub control: Decoder,
    pub pairs: Vec<(String, usize, Pairs)>,
    pub validation: BTreeMap<String, Report>,
    pub test: BTreeMap<String, Report>,
    pub recommendation: serde_json::Value,
}
pub fn run(teacher: &Decoder, initial: &Decoder, corpus: Corpus, study: Study) -> Result<Run> {
    study.verify(teacher, &corpus)?;
    if study.get(&study.recovery.start_plan)?.entries.len() < 2 {
        return Err("recovery requires multiple replacements".into());
    }
    let expected = plan::apply(teacher, &corpus, &study, &study.recovery.start_plan)?;
    if initial.model_id() != expected.model.model_id() {
        return Err("initial student is not the exact frozen start plan output".into());
    }
    let (student_arm, raw_student, student_pairs) =
        arm(teacher, initial, &corpus, &study, STUDENT)?;
    let (control_arm, raw_control, control_pairs) =
        arm(teacher, initial, &corpus, &study, CONTROL)?;
    let record = Record {
        schema: "nrl.recovery-record/0".into(),
        teacher_model_id: teacher.model_id().into(),
        initial_model_id: initial.model_id().into(),
        study: study.clone(),
        corpus: corpus.clone(),
        optimizer: "full-batch-local-sgd/0".into(),
        update_order: "all layers use the same frozen pre-round trajectory; apply together; no RNG"
            .into(),
        arms: vec![student_arm, control_arm],
    };
    let hash = record.record_id()?;
    let bind = |model: &Decoder, label: &str| -> Result<Decoder> {
        model.with_patches(model.patches(),&[Source{identifier:format!("derived://nrl-recovery/{label}#{hash}"),sha256:Some(hash.clone()),licence:corpus.source.licence.clone(),notices:vec!["Frozen original FFN teacher targets; actual train-only inputs; local SGD with full-model validation selection. Canonical recovery record hash; not end-to-end decoder backpropagation.".into()]}])
    };
    let student = bind(&raw_student, "student")?;
    let control = bind(&raw_control, "control")?;
    let mut validation = BTreeMap::new();
    for (label, model) in [
        ("dense", teacher),
        ("initial", initial),
        ("student", &student),
        ("control", &control),
    ] {
        validation.insert(
            label.into(),
            evaluate::evaluate_language(
                teacher,
                model,
                &corpus,
                "validation",
                Some(study.measurement.clone()),
            )?,
        );
    }
    let recommendation = recommend(&validation["dense"], &validation["student"], &study)?;
    let mut test = BTreeMap::new();
    for (label, model) in [
        ("dense", teacher),
        ("initial", initial),
        ("student", &student),
        ("control", &control),
    ] {
        test.insert(
            label.into(),
            evaluate::evaluate_language(teacher, model, &corpus, "test", None)?,
        );
    }
    let pairs = student_pairs
        .into_iter()
        .enumerate()
        .map(|(i, p)| ("student".to_string(), i + 1, p))
        .chain(
            control_pairs
                .into_iter()
                .enumerate()
                .map(|(i, p)| ("control".to_string(), i + 1, p)),
        )
        .collect();
    Ok(Run {
        record,
        initial: initial.clone(),
        student,
        control,
        pairs,
        validation,
        test,
        recommendation,
    })
}
impl Run {
    pub fn summary(&self) -> Result<serde_json::Value> {
        Ok(
            serde_json::json!({"schema":"nrl.recovery-summary/0","record_id":self.record.record_id()?,"teacher_model_id":self.record.teacher_model_id,"initial_model_id":self.initial.model_id(),"student_model_id":self.student.model_id(),"control_model_id":self.control.model_id(),"arms":self.record.arms,"recommendation":self.recommendation,"validation":self.validation,"test":self.test,"interpretation":"Bounded local recovery with simultaneous updates and complete-model validation; not end-to-end decoder gradients or a general quality/performance guarantee.","native_execution":false}),
        )
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        fs::create_dir(path).map_err(|e| e.to_string())?;
        self.record.save(&path.join("record.json"))?;
        self.initial.save(&path.join("initial-model"))?;
        self.student.save(&path.join("student-model"))?;
        self.control.save(&path.join("control-model"))?;
        for (label, round, pairs) in &self.pairs {
            save(
                &path.join(format!("{label}-round-{round}-pairs.json")),
                pairs,
            )?;
        }
        for (label, r) in &self.validation {
            r.save(&path.join(format!("{label}-validation.json")))?;
        }
        for (label, r) in &self.test {
            r.save(&path.join(format!("{label}-test.json")))?;
        }
        save(&path.join("complete.json"), &self.summary()?)
    }
}
pub fn replay(teacher: &Decoder, path: &Path) -> Result<Run> {
    let saved = Record::load(&path.join("record.json"))?;
    let complete: serde_json::Value =
        import_json::parse(&read(&path.join("complete.json"))?, MAX_BYTES)?;
    if complete["schema"] != "nrl.recovery-summary/0"
        || complete["record_id"] != saved.record_id()?
        || complete["arms"] != serde_json::to_value(&saved.arms).map_err(|e| e.to_string())?
        || complete["teacher_model_id"] != saved.teacher_model_id
        || complete["initial_model_id"] != saved.initial_model_id
        || complete["native_execution"] != false
    {
        return Err("incomplete or inconsistent saved recovery summary".into());
    }
    let initial = crate::decoder::load(&path.join("initial-model"))?;
    let repeated = run(teacher, &initial, saved.corpus.clone(), saved.study.clone())?;
    if repeated.record.record_id()? != saved.record_id()? {
        return Err("replayed recovery record/parameters differ on this execution profile".into());
    }
    for (label, round, pairs) in &repeated.pairs {
        let actual: Pairs = parse(
            &read(&path.join(format!("{label}-round-{round}-pairs.json")))?,
            "nrl.recovery-pairs/0",
        )?;
        if actual.pair_id()? != pairs.pair_id()? {
            return Err("saved recovery pairs differ from replay".into());
        }
    }
    for (label, model) in [
        ("student", &repeated.student),
        ("control", &repeated.control),
    ] {
        let loaded = crate::decoder::load(&path.join(format!("{label}-model")))?;
        if loaded.model_id() != model.model_id() {
            return Err("saved selected recovery model differs from replay".into());
        }
    }
    let mut saved_validation = BTreeMap::new();
    for (kind, reports) in [
        ("validation", &repeated.validation),
        ("test", &repeated.test),
    ] {
        for (label, expected) in reports {
            let actual = Report::load(&path.join(format!("{label}-{kind}.json")))?;
            if actual.profile != expected.profile
                || actual.model_id != expected.model_id
                || serde_json::to_value(&actual.quality).map_err(|e| e.to_string())?
                    != serde_json::to_value(&expected.quality).map_err(|e| e.to_string())?
                || actual.model_information != expected.model_information
                || complete[kind][label]
                    != serde_json::to_value(&actual).map_err(|e| e.to_string())?
            {
                return Err("saved recovery report differs from recomputed quality/model/profile or summary".into());
            }
            if kind == "validation" {
                saved_validation.insert(label.clone(), actual);
            }
        }
    }
    if complete["student_model_id"] != repeated.student.model_id()
        || complete["control_model_id"] != repeated.control.model_id()
        || complete["recommendation"]
            != recommend(
                &saved_validation["dense"],
                &saved_validation["student"],
                &saved.study,
            )?
    {
        return Err("saved recovery selection/measurement recommendation mismatch".into());
    }
    Ok(repeated)
}
