use nrl_lab::{
    decoder::Decoder,
    model::Source,
    research::{adaptation::Config, plan::Study, Corpus},
};
use serde_json::Value;
use std::collections::BTreeSet;
#[test]
fn frozen_larger_corpus_study_and_source_bindings_are_valid_on_the_real_tokenizer() {
    let v: Value = serde_json::from_slice(include_bytes!(
        "../../../reference/tinyllamas-260k/decoder-source.json"
    ))
    .unwrap();
    let sources: Vec<Source> = serde_json::from_value(v["sources"].clone()).unwrap();
    let m = Decoder::from_bytes(
        include_bytes!("../../../reference/tinyllamas-260k/stories260K.bin").to_vec(),
        include_bytes!("../../../reference/tinyllamas-260k/tok512.bin").to_vec(),
        sources,
    )
    .unwrap();
    let c = Corpus::parse(include_bytes!(
        "../../../tests/fixtures/expanded-v0/corpus.json"
    ))
    .unwrap();
    let study: Study = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/expanded-v0/study.json"
    ))
    .unwrap();
    study.verify(&m, &c).unwrap();
    let config: Config = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/expanded-v0/adaptation.json"
    ))
    .unwrap();
    config.validate().unwrap();
    assert_eq!(c.documents.len(), 48);
    assert_eq!(
        c.documents
            .iter()
            .map(|d| &d.group)
            .collect::<BTreeSet<_>>()
            .len(),
        48
    );
    for (split, count) in [("train", 24), ("validation", 12), ("test", 12)] {
        assert_eq!(
            c.documents.iter().filter(|d| d.split == split).count(),
            count
        );
    }
    assert_eq!(
        c.data_id().unwrap(),
        "25250ff6d3f6814d353f2d9b1ba130fce942c6bcc159e2e86f53a7dbc7aa1dfa"
    );
    assert!(c.source.licence.starts_with("CDLA-Sharing-1.0"));
    assert!(c
        .source
        .notices
        .iter()
        .any(|n| n.starts_with("MODIFIED DATA")));
    let t = c.tokenize(&m).unwrap();
    assert!(t.iter().map(|t| t.len() - 1).sum::<usize>() > 4096);
    assert_eq!(study.plans.len(), 8);
}
