use super::*;
use crate::{
    embedding::Encoder,
    model::{validate_sources, Source},
};
use evaluate::{Measurement, Quality, Report};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub id: String,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub id: String,
    pub group: String,
    pub split: String,
    pub text: String,
    pub relevance: BTreeMap<String, u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dataset {
    pub schema: String,
    pub source: Source,
    pub top_k: usize,
    pub documents: Vec<Document>,
    pub queries: Vec<Query>,
}
fn text(s: &str) -> bool {
    !s.is_empty() && s.len() <= 4096 && s.is_ascii() && s.bytes().any(|b| !b.is_ascii_whitespace())
}
impl Dataset {
    pub fn validate(&self) -> Result<()> {
        validate_sources(std::slice::from_ref(&self.source))?;
        if self.schema != "nrl.retrieval-data/0"
            || self.documents.is_empty()
            || self.documents.len() > 32
            || self.queries.is_empty()
            || self.queries.len() > 32
            || self.top_k == 0
            || self.top_k > self.documents.len()
        {
            return Err("unsupported retrieval dataset bounds/version".into());
        }
        let mut docs = BTreeSet::new();
        let mut texts = BTreeSet::new();
        for d in &self.documents {
            if !name(&d.id) || !docs.insert(&d.id) || !text(&d.text) || !texts.insert(&d.text) {
                return Err("duplicate/invalid retrieval document".into());
            }
        }
        let (mut ids, mut groups, mut query_texts, mut splits) = (
            BTreeSet::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeSet::new(),
        );
        for q in &self.queries {
            if !name(&q.id)
                || !name(&q.group)
                || !ids.insert(&q.id)
                || !text(&q.text)
                || !matches!(q.split.as_str(), "train" | "validation" | "test")
                || q.relevance.is_empty()
                || q.relevance
                    .iter()
                    .any(|(id, g)| !docs.contains(id) || !(1..=3).contains(g))
            {
                return Err("invalid retrieval query/group/split/relevance".into());
            }
            for previous in [
                groups.insert(&q.group, &q.split),
                query_texts.insert(&q.text, &q.split),
            ] {
                if previous.is_some_and(|s| s != &q.split) {
                    return Err("retrieval related group/query text crosses split".into());
                }
            }
            splits.insert(q.split.as_str());
        }
        if splits.len() != 3 {
            return Err("retrieval requires all three query splits".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let d: Self = parse(&read(path)?, "nrl.retrieval-data/0")?;
        d.validate()?;
        Ok(d)
    }
    pub fn data_id(&self) -> Result<String> {
        self.validate()?;
        id(self)
    }
    pub fn tokenize(&self, encoder: &Encoder) -> Result<(Vec<Vec<usize>>, Vec<Vec<usize>>)> {
        self.validate()?;
        let docs = self
            .documents
            .iter()
            .map(|d| encoder.tokenize(&d.text))
            .collect::<Result<Vec<_>>>()?;
        let qs = self
            .queries
            .iter()
            .map(|q| encoder.tokenize(&q.text))
            .collect::<Result<Vec<_>>>()?;
        let mut index = BTreeSet::new();
        for t in &docs {
            let mut bag = t.clone();
            bag.sort_unstable();
            if !index.insert(bag) {
                return Err("duplicate token bag in retrieval index".into());
            }
        }
        let mut seen = BTreeMap::new();
        for (q, t) in self.queries.iter().zip(&qs) {
            let mut bag = t.clone();
            bag.sort_unstable();
            if seen.insert(bag, &q.split).is_some_and(|s| s != &q.split) {
                return Err("order-invariant query token bag crosses split".into());
            }
        }
        Ok((docs, qs))
    }
}
pub fn evaluate(
    baseline: &Encoder,
    encoder: &Encoder,
    data: &Dataset,
    split: &str,
    spec: Option<Measurement>,
) -> Result<Report> {
    evaluate_inner(baseline, encoder, data, split, spec, false)
}
pub fn evaluate_trained(
    baseline: &Encoder,
    encoder: &Encoder,
    data: &Dataset,
    split: &str,
    spec: Option<Measurement>,
) -> Result<Report> {
    evaluate_inner(baseline, encoder, data, split, spec, true)
}
fn evaluate_inner(
    baseline: &Encoder,
    encoder: &Encoder,
    data: &Dataset,
    split: &str,
    spec: Option<Measurement>,
    trained: bool,
) -> Result<Report> {
    if if trained {
        baseline.input_contract_id()? != encoder.input_contract_id()?
    } else {
        baseline.input_content_id()? != encoder.input_content_id()?
    } {
        return Err("retrieval candidate input contract differs; projection-only profile also freezes table weights".into());
    }
    let (docs, queries) = data.tokenize(encoder)?;
    let selected: Vec<_> = data
        .queries
        .iter()
        .zip(queries)
        .filter(|(q, _)| q.split == split)
        .collect();
    if selected.is_empty() {
        return Err("empty/unknown retrieval split".into());
    }
    let rows = docs.len() + selected.len();
    let width = encoder.output_width()?;
    let profile = evaluate::profile(
        if trained {
            "embedding-trained-retrieval/0"
        } else {
            "embedding-retrieval/0"
        },
        data.data_id()?,
        split,
        baseline.model_id().into(),
        "exact-ascii-mean-projection-l2/0",
        vec![encoder.max_input_tokens(), width],
        spec,
        rows,
    )?;
    let env = evaluate::environment()?;
    let document_vectors = docs
        .iter()
        .map(|t| encoder.encode_tokens(t))
        .collect::<Result<Vec<_>>>()?;
    let (mut recalls, mut reciprocal, mut ndcgs, mut rankings) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (q, tokens) in &selected {
        let v = encoder.encode_tokens(tokens)?;
        let mut ranking: Vec<_> = data
            .documents
            .iter()
            .zip(&document_vectors)
            .map(|(d, e)| {
                (
                    d.id.clone(),
                    e.iter().zip(&v).map(|(a, b)| a * b).sum::<f64>(),
                )
            })
            .collect();
        ranking.sort_by(|(a, x), (b, y)| y.total_cmp(x).then(a.cmp(b)));
        let hits = ranking
            .iter()
            .take(data.top_k)
            .filter(|(id, _)| q.relevance.contains_key(id))
            .count();
        let recall = hits as f64 / q.relevance.len() as f64;
        let rr = ranking
            .iter()
            .position(|(id, _)| q.relevance.contains_key(id))
            .map_or(0.0, |i| 1.0 / (i + 1) as f64);
        let gain =
            |grade: u8, rank: usize| (2.0f64.powi(grade as i32) - 1.0) / ((rank + 2) as f64).log2();
        let dcg = ranking
            .iter()
            .take(data.top_k)
            .enumerate()
            .map(|(i, (id, _))| gain(*q.relevance.get(id).unwrap_or(&0), i))
            .sum::<f64>();
        let mut ideal: Vec<_> = q.relevance.values().copied().collect();
        ideal.sort_unstable_by(|a, b| b.cmp(a));
        let idcg = ideal
            .iter()
            .take(data.top_k)
            .enumerate()
            .map(|(i, g)| gain(*g, i))
            .sum::<f64>();
        let ndcg = dcg / idcg;
        finite(&[recall, rr, ndcg])?;
        recalls.push(recall);
        reciprocal.push(rr);
        ndcgs.push(ndcg);
        rankings.push(serde_json::json!({"query_id":q.id,"ranking":ranking,"recall_at_k":recall,"reciprocal_rank":rr,"ndcg_at_k":ndcg}));
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let ndcg = mean(&ndcgs);
    let quality = Quality {
        metric: "ndcg-loss-at-k".into(),
        value: 1.0 - ndcg.clamp(0.0, 1.0),
        details: serde_json::json!({"top_k":data.top_k,"recall_at_k":mean(&recalls),"mrr":mean(&reciprocal),"ndcg_at_k":ndcg,"direction":"lower loss is better; Recall/MRR/NDCG are higher-is-better","rankings":rankings,"ties":"descending f64 dot product, then lexical document ID","index_scope":"fixed document index shared across query splits; no fitting or selection"}),
    };
    let mut times = evaluate::benchmark(&profile.measurement, rows, || {
        for t in &docs {
            std::hint::black_box(encoder.encode_tokens(t)?);
        }
        for (_, t) in &selected {
            std::hint::black_box(encoder.encode_tokens(t)?);
        }
        Ok(())
    })?;
    if let Some(t) = &mut times {
        t.scope = evaluate::RETRIEVAL_TIMING_SCOPE.into();
    }
    evaluate::report(
        profile,
        encoder.model_id(),
        env,
        rows,
        rows,
        rows * width,
        quality,
        times,
        encoder.info()?,
    )
}
