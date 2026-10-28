use super::*;
use crate::{decoder::Decoder, model::Source};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub id: String,
    pub group: String,
    pub split: String,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub schema: String,
    pub source: Source,
    pub max_tokens_per_document: usize,
    pub documents: Vec<Document>,
}
impl Corpus {
    pub fn parse(b: &[u8]) -> Result<Self> {
        let data: Self = parse(b, "nrl.language-data/0")?;
        data.validate()?;
        Ok(data)
    }
    pub fn load(p: &Path) -> Result<Self> {
        Self::parse(&read(p)?)
    }
    pub fn data_id(&self) -> Result<String> {
        self.validate()?;
        id(self)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != "nrl.language-data/0" {
            return Err(format!("unsupported corpus schema: {}", self.schema));
        }
        crate::model::validate_sources(std::slice::from_ref(&self.source))?;
        if self.documents.is_empty()
            || self.documents.len() > 64
            || self.max_tokens_per_document < 2
            || self.max_tokens_per_document > 512
        {
            return Err("corpus document/context bound".into());
        }
        let mut ids = BTreeSet::new();
        let mut groups = BTreeMap::new();
        let mut texts = BTreeMap::new();
        let mut splits = BTreeSet::new();
        for d in &self.documents {
            if !name(&d.id)
                || !name(&d.group)
                || !ids.insert(&d.id)
                || !matches!(d.split.as_str(), "train" | "validation" | "test")
                || d.text.is_empty()
                || d.text.len() > 4096
                || d.text.contains('\0')
            {
                return Err("invalid corpus ID/group/split/text".into());
            }
            if groups
                .insert(&d.group, &d.split)
                .is_some_and(|v| v != &d.split)
            {
                return Err("corpus related group crosses splits".into());
            }
            if texts
                .insert(&d.text, &d.split)
                .is_some_and(|v| v != &d.split)
            {
                return Err("corpus duplicate text crosses splits".into());
            }
            splits.insert(&d.split);
        }
        if splits.len() != 3 {
            return Err("corpus needs nonempty train/validation/test".into());
        }
        Ok(())
    }
    pub fn tokenize(&self, model: &Decoder) -> Result<Vec<Vec<usize>>> {
        self.validate()?;
        if self.max_tokens_per_document > model.config().context {
            return Err("corpus context exceeds model profile".into());
        }
        let mut parts = BTreeMap::new();
        let mut result = Vec::new();
        for d in &self.documents {
            let ids = model.tokenizer.encode(&d.text, true, false)?;
            if ids.len() < 2 || ids.len() > self.max_tokens_per_document {
                return Err(format!(
                    "document {} outside explicit token/context limit",
                    d.id
                ));
            }
            if parts
                .insert(ids.clone(), &d.split)
                .is_some_and(|v| v != &d.split)
            {
                return Err("duplicate token sequence crosses corpus splits".into());
            }
            result.push(ids);
        }
        Ok(result)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairDocument {
    pub document_id: String,
    pub group: String,
    pub tokens: Vec<usize>,
    pub input: Vec<f64>,
    pub target: Vec<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Calibration {
    pub schema: String,
    pub teacher_model_id: String,
    pub corpus: Corpus,
    pub corpus_id: String,
    pub layer: usize,
    pub dim: usize,
    pub protocol: String,
    pub row_limit: usize,
    pub train: Vec<PairDocument>,
    pub validation: Vec<PairDocument>,
}
impl Calibration {
    pub fn collect(
        model: &Decoder,
        corpus: Corpus,
        layer: usize,
        row_limit: usize,
    ) -> Result<Self> {
        let tokenized = corpus.tokenize(model)?;
        model.dense(layer)?;
        if row_limit == 0 || row_limit > 256 || !model.patches().is_empty() {
            return Err("original-teacher calibration row/profile limit".into());
        }
        let mut c = Self {
            schema: "nrl.calibration/0".into(),
            teacher_model_id: model.model_id().into(),
            corpus_id: corpus.data_id()?,
            dim: model.config().dim,
            layer,
            protocol: "frozen-original-teacher-input/0".into(),
            row_limit,
            train: vec![],
            validation: vec![],
            corpus,
        };
        for split in ["train", "validation"] {
            let mut remaining = row_limit;
            for (doc, tokens) in c
                .corpus
                .documents
                .iter()
                .zip(&tokenized)
                .filter(|(d, _)| d.split == split)
            {
                if remaining == 0 {
                    break;
                }
                let count = (tokens.len() - 1).min(remaining);
                let input_tokens = tokens[..count].to_vec();
                let trace = model.trace(&input_tokens)?;
                let row = &trace.layers[layer];
                let pair = PairDocument {
                    document_id: doc.id.clone(),
                    group: doc.group.clone(),
                    tokens: input_tokens,
                    input: row.input.clone(),
                    target: row.output.clone(),
                };
                if split == "train" {
                    c.train.push(pair);
                } else {
                    c.validation.push(pair);
                }
                remaining -= count;
            }
        }
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<()> {
        self.corpus.validate()?;
        if self.schema != "nrl.calibration/0"
            || !storage::valid_hash(&self.teacher_model_id)
            || self.corpus_id != self.corpus.data_id()?
            || self.protocol != "frozen-original-teacher-input/0"
            || self.dim == 0
            || self.dim > 128
            || self.layer >= 12
            || self.row_limit == 0
            || self.row_limit > 256
        {
            return Err("calibration identity/profile/bound mismatch".into());
        }
        for (split, part) in [("train", &self.train), ("validation", &self.validation)] {
            let mut rows = 0;
            let mut ids = BTreeSet::new();
            for pair in part {
                let doc = self
                    .corpus
                    .documents
                    .iter()
                    .find(|d| d.id == pair.document_id)
                    .ok_or("unknown calibration document")?;
                if doc.split != split
                    || doc.group != pair.group
                    || !ids.insert(&pair.document_id)
                    || pair.tokens.is_empty()
                    || pair.tokens.len() >= self.corpus.max_tokens_per_document
                    || pair.tokens.iter().any(|i| *i >= 1024)
                    || pair.input.len() != pair.tokens.len() * self.dim
                    || pair.target.len() != pair.input.len()
                {
                    return Err("calibration split/document/token/shape mismatch".into());
                }
                finite(&pair.input)?;
                finite(&pair.target)?;
                rows += pair.tokens.len();
            }
            if rows == 0 || rows > self.row_limit {
                return Err("calibration row bound".into());
            }
        }
        Ok(())
    }
    pub fn verify_teacher(&self, model: &Decoder) -> Result<()> {
        self.validate()?;
        if !model.patches().is_empty()
            || self.teacher_model_id != model.model_id()
            || self.dim != model.config().dim
            || self.layer >= model.config().layers
        {
            return Err("calibration belongs to another teacher".into());
        }
        let tokens = self.corpus.tokenize(model)?;
        for pair in self.train.iter().chain(&self.validation) {
            let index = self
                .corpus
                .documents
                .iter()
                .position(|d| d.id == pair.document_id)
                .unwrap();
            if pair.tokens.len() >= tokens[index].len()
                || pair.tokens != tokens[index][..pair.tokens.len()]
            {
                return Err("calibration token provenance mismatch".into());
            }
        }
        for pair in self.train.iter().chain(&self.validation) {
            let trace = model.trace(&pair.tokens)?;
            crate::close(&pair.input, &trace.layers[self.layer].input)?;
            crate::close(&pair.target, &trace.layers[self.layer].output)?;
        }
        Ok(())
    }
    pub fn batch(&self, split: &str) -> Result<(Vec<f64>, Vec<f64>)> {
        self.validate()?;
        let part = match split {
            "train" => &self.train,
            "validation" => &self.validation,
            _ => return Err("calibration has no test activations".into()),
        };
        Ok((
            part.iter().flat_map(|p| p.input.iter().copied()).collect(),
            part.iter().flat_map(|p| p.target.iter().copied()).collect(),
        ))
    }
    pub fn calibration_id(&self) -> Result<String> {
        self.validate()?;
        id(self)
    }
    pub fn load(p: &Path) -> Result<Self> {
        let c: Self = parse(&read(p)?, "nrl.calibration/0")?;
        c.validate()?;
        Ok(c)
    }
    pub fn save(&self, p: &Path) -> Result<()> {
        self.validate()?;
        save(p, self)
    }
}
