use super::*;
use crate::{decoder::Decoder, experiment::Dataset, model::Model};
use serde::{Deserialize, Serialize};
use std::{fs, sync::OnceLock, time::Instant};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub schema: String,
    pub warmups: usize,
    pub repeats: usize,
    pub max_forward_rows: usize,
}
impl Measurement {
    pub fn validate(&self) -> Result<()> {
        if self.schema != "nrl.measurement/0"
            || !(1..=3).contains(&self.warmups)
            || !(3..=9).contains(&self.repeats)
            || self.max_forward_rows == 0
            || self.max_forward_rows > 65536
        {
            return Err("unsupported measurement warmup/repeat/work profile".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let p: Self = parse(&read(path)?, "nrl.measurement/0")?;
        p.validate()?;
        Ok(p)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub os: String,
    pub architecture: String,
    pub cpu: Option<String>,
    pub logical_cpus: usize,
    pub executable_sha256: String,
    pub boot_sha256: Option<String>,
    pub backend: String,
    pub runtime_threads: usize,
    pub debug_assertions: bool,
}
pub(crate) fn environment() -> Result<Environment> {
    static ENV: OnceLock<Result<Environment>> = OnceLock::new();
    ENV.get_or_init(|| {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let cpu = fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("model name")
                    .and_then(|v| v.split_once(':'))
                    .map(|(_, v)| v.trim().chars().take(256).collect())
            })
        });
        let boot = fs::read("/proc/sys/kernel/random/boot_id")
            .ok()
            .map(|b| storage::hash(&b));
        Ok(Environment {
            os: std::env::consts::OS.into(),
            architecture: std::env::consts::ARCH.into(),
            cpu,
            logical_cpus: std::thread::available_parallelism().map_or(0, |n| n.get()),
            executable_sha256: storage::hash_file(&executable, 128 << 20)?,
            boot_sha256: boot,
            backend: "host-cpu-scalar-f64/0".into(),
            runtime_threads: 1,
            debug_assertions: cfg!(debug_assertions),
        })
    })
    .clone()
}
fn process_peak_rss() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|l| {
        l.strip_prefix("VmHWM:")
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<u64>().ok())
            .and_then(|n| n.checked_mul(1024))
    })
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub task: String,
    pub data_id: String,
    pub split: String,
    pub baseline_id: String,
    pub preprocessing: String,
    pub geometry: Vec<usize>,
    pub measurement: Option<Measurement>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Work {
    pub documents_or_batches: usize,
    pub rows_per_pass: usize,
    pub output_values_per_pass: usize,
    pub quality_passes: usize,
    pub warmup_passes: usize,
    pub timed_passes: usize,
    pub total_forward_rows: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    pub seconds: Vec<f64>,
    pub minimum_seconds: f64,
    pub median_seconds: f64,
    pub p95_seconds: f64,
    pub scope: String,
}
pub(crate) fn timing(seconds: Vec<f64>) -> Result<Timing> {
    finite(&seconds)?;
    if seconds.is_empty() || seconds.iter().any(|v| *v < 0.0) {
        return Err("invalid timing samples".into());
    }
    let mut sorted = seconds.clone();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    Ok(Timing{seconds,minimum_seconds:sorted[0],median_seconds:sorted[n/2],p95_seconds:sorted[(n*95).div_ceil(100)-1],scope:"preprocessed forward only, fresh per-document cache; allocations included; quality/hash/load excluded".into()})
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quality {
    pub metric: String,
    pub value: f64,
    pub details: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub profile: Profile,
    pub model_id: String,
    pub environment: Environment,
    pub work: Work,
    pub quality: Quality,
    pub timing: Option<Timing>,
    pub model_information: serde_json::Value,
    pub process_peak_rss_bytes: Option<u64>,
    pub memory_scope: String,
    pub native_execution: bool,
}
impl Report {
    pub fn validate(&self) -> Result<()> {
        finite(&[self.quality.value])?;
        if self.schema != "nrl.evaluation/0"
            || self.native_execution
            || !storage::valid_hash(&self.model_id)
            || !storage::valid_hash(&self.profile.baseline_id)
            || !storage::valid_hash(&self.profile.data_id)
            || !storage::valid_hash(&self.environment.executable_sha256)
            || self.environment.backend != "host-cpu-scalar-f64/0"
            || self.environment.runtime_threads != 1
            || !matches!(self.profile.split.as_str(), "train" | "validation" | "test")
            || self.quality.value < 0.0
            || !(1..=65536).contains(&self.work.rows_per_pass)
            || !(1..=64).contains(&self.work.documents_or_batches)
            || self.work.quality_passes != 1
            || self.profile.geometry.len() != 2
            || self.profile.geometry.iter().any(|v| !(1..=4096).contains(v))
            || self.environment.boot_sha256.as_ref().is_some_and(|v| !storage::valid_hash(v))
            || self.memory_scope != "whole-process Linux VmHWM, including previously loaded models/work; not per-model allocation or savings"
            || self.work.output_values_per_pass != self.work.rows_per_pass * if self.profile.task == "language-next-token/0" { self.profile.geometry[0] } else { self.profile.geometry[1] }
        {
            return Err("evaluation identity/task/work/profile mismatch".into());
        }
        match self.profile.task.as_str() {
            "regression-vector/0"
                if self.quality.metric == "mse"
                    && self.profile.preprocessing == "identity-f64/0" =>
            {
                ()
            }
            "language-next-token/0"
                if self.quality.metric == "mean-nll"
                    && self.profile.preprocessing == "byte-bpe-bos-shifted-no-eos/0"
                    && (259..=1024).contains(&self.profile.geometry[0])
                    && (2..=512).contains(&self.profile.geometry[1]) =>
            {
                ()
            }
            "embedding-retrieval/0" | "embedding-trained-retrieval/0"
                if self.quality.metric == "ndcg-loss-at-k"
                    && self.quality.value <= 1.0
                    && self.profile.preprocessing == "exact-ascii-mean-projection-l2/0"
                    && self.profile.geometry[0] <= 128
                    && self.profile.geometry[1] <= 128 =>
            {
                ()
            }
            _ => return Err("unsupported evaluation task/metric/preprocessing".into()),
        }
        if let Some(spec) = &self.profile.measurement {
            spec.validate()?;
            let t = self.timing.as_ref().ok_or("missing measured times")?;
            let mut expected = timing(t.seconds.clone())?;
            if matches!(
                self.profile.task.as_str(),
                "embedding-retrieval/0" | "embedding-trained-retrieval/0"
            ) {
                expected.scope = RETRIEVAL_TIMING_SCOPE.into();
            }
            if t.seconds.len() != spec.repeats
                || t.minimum_seconds != expected.minimum_seconds
                || t.median_seconds != expected.median_seconds
                || t.p95_seconds != expected.p95_seconds
                || t.scope != expected.scope
                || self.work.warmup_passes != spec.warmups
                || self.work.timed_passes != spec.repeats
                || self.work.total_forward_rows > spec.max_forward_rows
            {
                return Err("measurement timing/work record mismatch".into());
            }
        } else if self.timing.is_some()
            || self.work.warmup_passes != 0
            || self.work.timed_passes != 0
        {
            return Err("quality-only report has measurement work".into());
        }
        if self.work.total_forward_rows
            != self.work.rows_per_pass * (1 + self.work.warmup_passes + self.work.timed_passes)
        {
            return Err("evaluation work total mismatch".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let r: Self = parse(&read(path)?, "nrl.evaluation/0")?;
        r.validate()?;
        Ok(r)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        save(path, self)
    }
    pub fn report_id(&self) -> Result<String> {
        self.validate()?;
        id(self)
    }
}
pub(crate) fn benchmark<F: FnMut() -> Result<()>>(
    spec: &Option<Measurement>,
    rows: usize,
    mut forward: F,
) -> Result<Option<Timing>> {
    if let Some(spec) = spec {
        spec.validate()?;
        if rows * (1 + spec.warmups + spec.repeats) > spec.max_forward_rows {
            return Err("measurement forward-row work budget exceeded".into());
        }
        for _ in 0..spec.warmups {
            forward()?;
        }
        let mut seconds = Vec::new();
        for _ in 0..spec.repeats {
            let start = Instant::now();
            forward()?;
            seconds.push(start.elapsed().as_secs_f64());
        }
        Ok(Some(timing(seconds)?))
    } else {
        Ok(None)
    }
}
pub(crate) fn profile(
    task: &str,
    data_id: String,
    split: &str,
    baseline: String,
    preprocessing: &str,
    geometry: Vec<usize>,
    spec: Option<Measurement>,
    rows: usize,
) -> Result<Profile> {
    if !matches!(split, "train" | "validation" | "test") {
        return Err("unknown evaluation split".into());
    }
    if let Some(s) = &spec {
        s.validate()?;
        if rows * (1 + s.warmups + s.repeats) > s.max_forward_rows {
            return Err("evaluation forward-row work budget exceeded".into());
        }
    }
    Ok(Profile {
        task: task.into(),
        data_id,
        split: split.into(),
        baseline_id: baseline,
        preprocessing: preprocessing.into(),
        geometry,
        measurement: spec,
    })
}
pub(crate) fn report(
    profile: Profile,
    model_id: &str,
    env: Environment,
    rows: usize,
    batches: usize,
    output_values: usize,
    quality: Quality,
    timing: Option<Timing>,
    information: serde_json::Value,
) -> Result<Report> {
    let (warmups, repeats) = profile
        .measurement
        .as_ref()
        .map_or((0, 0), |s| (s.warmups, s.repeats));
    let r=Report{schema:"nrl.evaluation/0".into(),profile,model_id:model_id.into(),environment:env,work:Work{documents_or_batches:batches,rows_per_pass:rows,output_values_per_pass:output_values,quality_passes:1,warmup_passes:warmups,timed_passes:repeats,total_forward_rows:rows*(1+warmups+repeats)},quality,timing,model_information:information,process_peak_rss_bytes:process_peak_rss(),memory_scope:"whole-process Linux VmHWM, including previously loaded models/work; not per-model allocation or savings".into(),native_execution:false};
    r.validate()?;
    Ok(r)
}
pub fn language_quality(
    model: &Decoder,
    corpus: &Corpus,
    split: &str,
) -> Result<(Quality, usize, usize)> {
    let tokenized = corpus.tokenize(model)?;
    let batches: Vec<_> = corpus
        .documents
        .iter()
        .zip(tokenized)
        .filter(|(d, _)| d.split == split)
        .collect();
    if batches.is_empty() {
        return Err("empty/unknown language evaluation split".into());
    }
    let mut losses = Vec::new();
    let mut correct = 0;
    for (_, tokens) in &batches {
        let logits = model.forward(&tokens[..tokens.len() - 1])?;
        for (row, target) in logits.chunks_exact(model.config().vocab).zip(&tokens[1..]) {
            let max = row.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let lse = max + row.iter().map(|v| (v - max).exp()).sum::<f64>().ln();
            let loss = lse - row[*target];
            finite(&[loss])?;
            losses.push(loss);
            let mut best = 0;
            for (i, v) in row.iter().enumerate().skip(1) {
                if *v > row[best] {
                    best = i;
                }
            }
            if best == *target {
                correct += 1;
            }
        }
    }
    let mean = losses.iter().sum::<f64>() / losses.len() as f64;
    let perplexity = mean.exp();
    finite(&[mean, perplexity])?;
    let mut sorted = losses.clone();
    sorted.sort_by(f64::total_cmp);
    Ok((
        Quality {
            metric: "mean-nll".into(),
            value: mean,
            details: serde_json::json!({"perplexity":perplexity,"predicted_tokens":losses.len(),"top1_accuracy":correct as f64/losses.len()as f64,"p95_token_nll":sorted[(sorted.len()*95).div_ceil(100)-1],"maximum_token_nll":sorted[sorted.len()-1],"state":"fresh causal cache per document"}),
        },
        losses.len(),
        batches.len(),
    ))
}
pub fn evaluate_language(
    baseline: &Decoder,
    model: &Decoder,
    corpus: &Corpus,
    split: &str,
    spec: Option<Measurement>,
) -> Result<Report> {
    if baseline.base_content_id()? != model.base_content_id()? {
        return Err(
            "language candidate has another source/config/tokenizer/numerical baseline".into(),
        );
    }
    let tokenized = corpus.tokenize(model)?;
    let inputs: Vec<_> = corpus
        .documents
        .iter()
        .zip(tokenized)
        .filter(|(d, _)| d.split == split)
        .map(|(_, t)| t[..t.len() - 1].to_vec())
        .collect();
    let rows = inputs.iter().map(Vec::len).sum();
    let profile = profile(
        "language-next-token/0",
        corpus.data_id()?,
        split,
        baseline.model_id().into(),
        "byte-bpe-bos-shifted-no-eos/0",
        vec![model.config().vocab, corpus.max_tokens_per_document],
        spec,
        rows,
    )?;
    let env = environment()?;
    let (quality, count, batches) = language_quality(model, corpus, split)?;
    let times = benchmark(&profile.measurement, count, || {
        for input in &inputs {
            std::hint::black_box(model.forward(input)?);
        }
        Ok(())
    })?;
    report(
        profile,
        model.model_id(),
        env,
        count,
        batches,
        count * model.config().vocab,
        quality,
        times,
        model.info()?,
    )
}
pub fn evaluate_regression(
    baseline: &Model,
    model: &Model,
    data: &Dataset,
    split: &str,
    spec: Option<Measurement>,
) -> Result<Report> {
    data.validate()?;
    let n = data.input_width;
    let m = data.output_width;
    for candidate in [baseline, model] {
        let info = candidate.info()?;
        if info["input_width"].as_u64() != Some(n as u64)
            || info["output_width"].as_u64() != Some(m as u64)
            || !candidate
                .manifest()
                .descriptor
                .tasks
                .iter()
                .any(|t| t == "regression-vector/0")
        {
            return Err("incompatible regression model/data task".into());
        }
    }
    let selected: Vec<_> = data.samples.iter().filter(|s| s.split == split).collect();
    let input: Vec<_> = selected.iter().flat_map(|s| s.input.clone()).collect();
    let target: Vec<_> = selected.iter().flat_map(|s| s.target.clone()).collect();
    let rows = selected.len();
    if rows == 0 {
        return Err("empty regression split".into());
    }
    let profile = profile(
        "regression-vector/0",
        data.data_id()?,
        split,
        baseline.model_id().into(),
        "identity-f64/0",
        vec![n, m],
        spec,
        rows,
    )?;
    let env = environment()?;
    let output = model.forward(&input)?;
    let count = target.len() as f64;
    let mut mse = 0.0;
    let mut mae = 0.0;
    let mut energy = 0.0;
    let mut maximum: f64 = 0.0;
    for (a, b) in output.iter().zip(&target) {
        let error = (a - b).abs();
        mse += error * error / count;
        mae += error / count;
        energy += b * b / count;
        maximum = maximum.max(error);
    }
    let nmse = mse / energy.max(1e-12);
    finite(&[mse, mae, energy, maximum, nmse])?;
    let quality = Quality {
        metric: "mse".into(),
        value: mse,
        details: serde_json::json!({"mae":mae,"nmse":nmse,"target_mean_energy":energy,"near_zero_target_energy":energy<=1e-12,"maximum_absolute_error":maximum,"output_values":target.len()}),
    };
    let times = benchmark(&profile.measurement, rows, || {
        std::hint::black_box(model.forward(&input)?);
        Ok(())
    })?;
    report(
        profile,
        model.model_id(),
        env,
        rows,
        1,
        target.len(),
        quality,
        times,
        model.info()?,
    )
}
pub fn compare(baseline: &Report, candidate: &Report) -> Result<serde_json::Value> {
    baseline.validate()?;
    candidate.validate()?;
    if baseline.profile != candidate.profile
        || baseline.environment != candidate.environment
        || baseline.quality.metric != candidate.quality.metric
        || baseline.work.rows_per_pass != candidate.work.rows_per_pass
        || baseline.work.output_values_per_pass != candidate.work.output_values_per_pass
        || baseline.work.documents_or_batches != candidate.work.documents_or_batches
    {
        return Err("incompatible evaluation task/data/split/baseline/geometry/host/executable/measurement/work".into());
    }
    if baseline.timing.is_some() && baseline.environment.boot_sha256.is_none() {
        return Err("timing comparison requires recorded host boot identity".into());
    }
    let ratio = match (&baseline.timing, &candidate.timing) {
        (Some(a), Some(b)) if a.median_seconds > 0.0 => Some(b.median_seconds / a.median_seconds),
        _ => None,
    };
    if ratio.is_some_and(|v| !v.is_finite()) {
        return Err("non-finite timing ratio".into());
    }
    Ok(
        serde_json::json!({"schema":"nrl.comparison/0","baseline_report_id":baseline.report_id()?,"candidate_report_id":candidate.report_id()?,"metric":baseline.quality.metric,"quality_delta":candidate.quality.value-baseline.quality.value,"candidate_to_baseline_median_time_ratio":ratio,"baseline_package_bytes":baseline.model_information["package_bytes"],"candidate_package_bytes":candidate.model_information["package_bytes"],"process_peak_rss_not_attributed_to_model":true,"native_execution":false,"interpretation":"Compatible recorded task/host configuration only; ratios and parameter counts are not a general speed or memory guarantee."}),
    )
}
pub fn save_comparison(path: &Path, baseline: &Report, candidate: &Report) -> Result<()> {
    save(path, &compare(baseline, candidate)?)
}

pub(crate) const RETRIEVAL_TIMING_SCOPE:&str="pretokenized stateless encoder forward only; pooling/projection/normalization included; ranking/quality/hash/load excluded";
