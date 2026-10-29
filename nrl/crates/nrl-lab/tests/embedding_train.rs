use nrl_lab::{
    close,
    embedding::{Definition, Encoder},
    optimizer::Tensors,
    research::{
        embedding_train::{self, Config, State},
        retrieval,
    },
};
use serde_json::Value;
fn encoder() -> Encoder {
    let d: Definition = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/embedding-v0/definition.json"
    ))
    .unwrap();
    Encoder::from_definition(d).unwrap()
}
fn data() -> retrieval::Dataset {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/embedding-v0/data.json"
    ))
    .unwrap()
}
fn config() -> Config {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/embedding-training-v0/config.json"
    ))
    .unwrap()
}
fn oracle() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/embedding-training-v0/oracle.json"
    ))
    .unwrap()
}
fn check(a: &Tensors, b: &Tensors) {
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (k, v) in a {
        if !v.is_empty() {
            close(v, &b[k]).unwrap();
        }
    }
}
#[test]
fn lookup_mean_projection_normalization_and_contrastive_gradients_match_autograd() {
    let e = encoder();
    let f = oracle();
    let b = &f["backward"];
    let dy: Vec<f64> = serde_json::from_value(b["dy"].clone()).unwrap();
    let t = e.tokenize(b["text"].as_str().unwrap()).unwrap();
    let actual = e.backward_tokens(&t, &dy, 65536).unwrap();
    close(
        &actual.output,
        &serde_json::from_value::<Vec<f64>>(b["output"].clone()).unwrap(),
    )
    .unwrap();
    check(
        &actual.gradients,
        &serde_json::from_value(b["gradients"].clone()).unwrap(),
    );
    let (loss, g) = embedding_train::objective(&e, &data(), "train", &config(), true).unwrap();
    close(&[loss], &[f["contrastive"]["loss"].as_f64().unwrap()]).unwrap();
    check(
        &g,
        &serde_json::from_value(f["contrastive"]["gradients"].clone()).unwrap(),
    );
    assert!(embedding_train::objective(&e, &data(), "test", &config(), true).is_err());
    assert!(e.backward_tokens(&t, &dy, 1).is_err());
    assert!(e.backward_tokens(&t, &[f64::NAN; 3], 65536).is_err());
}
#[test]
fn adamw_training_matches_external_steps_and_exact_resumed_state() {
    let e = encoder();
    let d = data();
    let mut c = config();
    c.optimizer.max_steps = 2;
    let f = oracle();
    let start = State::start(&e, d.clone(), c.clone()).unwrap();
    let mut full = start.clone();
    for (i, row) in f["history"].as_array().unwrap().iter().enumerate() {
        full.advance(i + 1).unwrap();
        check(
            &full.current,
            &serde_json::from_value(row["parameters"].clone()).unwrap(),
        );
        close(
            &[
                full.history[i + 1].train_loss,
                full.history[i + 1].validation_loss,
            ],
            &[
                row["train_loss"].as_f64().unwrap(),
                row["validation_loss"].as_f64().unwrap(),
            ],
        )
        .unwrap();
    }
    let mut partial = start;
    partial.advance(1).unwrap();
    let path =
        std::env::temp_dir().join(format!("nrl-embedding-train-{}.json", std::process::id()));
    partial.save(&path).unwrap();
    let mut resumed = State::load(&path).unwrap();
    resumed.advance(2).unwrap();
    assert_eq!(
        serde_json::to_value(&resumed).unwrap(),
        serde_json::to_value(&full).unwrap()
    );
    let mut changed = d;
    for q in &mut changed.queries {
        if q.split == "test" {
            q.text.push_str(" bright");
        }
    }
    let mut other = State::start(&e, changed, c).unwrap();
    other.advance(2).unwrap();
    assert_eq!(other.current, full.current);
    assert_eq!(other.best, full.best);
    assert_eq!(other.best_step, full.best_step);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn trained_model_roundtrip_uses_explicit_table_training_profile() {
    let e = encoder();
    let mut c = config();
    c.optimizer.max_steps = 2;
    let mut state = State::start(&e, data(), c).unwrap();
    state.advance(2).unwrap();
    let selected = state.export().unwrap();
    assert_ne!(e.parameters().unwrap()["table"], state.current["table"]);
    assert_ne!(
        e.input_content_id().unwrap(),
        selected.input_content_id().unwrap()
    );
    assert_eq!(
        e.input_contract_id().unwrap(),
        selected.input_contract_id().unwrap()
    );
    assert!(retrieval::evaluate(&e, &selected, &data(), "test", None).is_err());
    assert_eq!(
        retrieval::evaluate_trained(&e, &selected, &data(), "test", None)
            .unwrap()
            .profile
            .task,
        "embedding-trained-retrieval/0"
    );
    let mut def = e.definition();
    def.table.tokens.swap(0, 1);
    let wrong = Encoder::from_definition(def).unwrap();
    assert!(retrieval::evaluate_trained(&e, &wrong, &data(), "test", None).is_err());
    let path = std::env::temp_dir().join(format!("nrl-embedding-trained-{}", std::process::id()));
    selected.save(&path).unwrap();
    let restored = nrl_lab::embedding::load(&path).unwrap();
    assert_eq!(restored.model_id(), selected.model_id());
    assert_eq!(
        restored.encode("fox fox red").unwrap(),
        selected.encode("fox fox red").unwrap()
    );
    std::fs::remove_dir_all(path).unwrap();
}
