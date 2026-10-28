use nrl_lab::{
    close,
    model::{Definition, Model},
    runtime_bridge::{self, Policy, Prepared},
};
use nrl_runtime::{
    Budget, Checkpoint, Control, Error, Kernel, Operation, Program, Session, Status,
};
struct Clock {
    tick: u64,
    calls: u64,
    cancel_at: Option<u64>,
    reverse: bool,
}
impl Control for Clock {
    fn checkpoint(&mut self) -> Checkpoint {
        self.calls += 1;
        let t = if self.reverse && self.calls > 1 {
            self.tick - 1
        } else {
            self.tick
        };
        if !self.reverse {
            self.tick += 1;
        }
        Checkpoint {
            tick: t,
            cancelled: self.cancel_at == Some(self.calls),
        }
    }
}
fn clock() -> Clock {
    Clock {
        tick: 0,
        calls: 0,
        cancel_at: None,
        reverse: false,
    }
}
struct Double(bool);
impl Kernel for Double {
    fn input_width(&self) -> usize {
        1
    }
    fn output_width(&self) -> usize {
        1
    }
    fn scratch_values(&self) -> usize {
        0
    }
    fn work_units(&self) -> u64 {
        2
    }
    fn forward(&self, x: &[f64], y: &mut [f64], _: &mut [f64]) -> nrl_core::Result<()> {
        if self.0 {
            return Err(nrl_core::Error::NonFinite);
        }
        y[0] = 2.0 * x[0];
        Ok(())
    }
}
fn program<'a>(nodes: &'a [Operation<'a>]) -> Program<'a> {
    Program {
        version: 0,
        model_id: [1; 32],
        input_width: 1,
        max_batch_rows: 16,
        max_batch_values: 1024,
        nodes,
        output: 1,
    }
}
fn budget() -> Budget {
    Budget {
        max_rows: 4,
        max_work_units: 8,
        max_workspace_values: 100,
        max_output_values: 16,
        max_requests: 2,
        deadline_tick: 100,
    }
}
#[test]
fn borrowed_graph_matches_all_five_paths_and_shared_objects() {
    let m = Model::from_definition(
        Definition::parse(include_bytes!(
            "../../../tests/fixtures/model-v0/definition.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let x: Vec<f64> = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/model-v0/input.json"
    ))
    .unwrap();
    let p: Policy = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/runtime-v0/policy.json"
    ))
    .unwrap();
    let r = runtime_bridge::run(&m, &x, &p).unwrap();
    let y: Vec<f64> = serde_json::from_value(r["output"].clone()).unwrap();
    close(&y, &m.forward(&x).unwrap()).unwrap();
    assert_eq!(r["workspace_cleared"], true);
    assert_eq!(r["native_execution"], false);
    let prepared = Prepared::new(&m).unwrap();
    let ops = prepared.operations().unwrap();
    let pg = prepared.program(&ops).unwrap();
    assert!(pg.layout().unwrap().work_units_per_row > 0);
}
#[test]
fn session_identity_cumulative_budget_and_model_limits_are_enforced() {
    let k = Double(false);
    let nodes = [Operation::Kernel {
        input: 0,
        kernel: &k,
    }];
    let p = program(&nodes);
    let mut s = Session::new(42, p.model_id, budget()).unwrap();
    let mut c = clock();
    let mut y = [9.; 2];
    let mut w = [9.; 2];
    assert_eq!(
        s.execute(7, &p, &[1., 2.], &mut y, &mut w, &mut c),
        Err(Error::Owner)
    );
    assert_eq!(y, [0.; 2]);
    assert_eq!(w, [0.; 2]);
    assert_eq!(c.calls, 0);
    let mut other = program(&nodes);
    other.model_id = [2; 32];
    assert_eq!(
        s.execute(42, &other, &[1., 2.], &mut y, &mut w, &mut c),
        Err(Error::Model)
    );
    assert_eq!(s.usage().requests, 0);
    s.execute(42, &p, &[1., 2.], &mut y, &mut w, &mut c)
        .unwrap();
    assert_eq!(y, [2., 4.]);
    s.execute(42, &p, &[1., 2.], &mut y, &mut w, &mut c)
        .unwrap();
    assert_eq!(s.usage().rows, 4);
    assert_eq!(s.usage().work_units, 8);
    assert_eq!(s.usage().requests, 2);
    assert_eq!(
        s.execute(42, &p, &[1., 2.], &mut y, &mut w, &mut c),
        Err(Error::Limit)
    );
    assert_eq!(y, [0.; 2]);
    assert_eq!(s.status(), Status::Ready);
    let mut narrow = program(&nodes);
    narrow.max_batch_rows = 1;
    let mut s = Session::new(42, p.model_id, budget()).unwrap();
    assert_eq!(
        s.execute(42, &narrow, &[1., 2.], &mut y, &mut w, &mut c),
        Err(Error::Limit)
    );
    assert_eq!(s.usage().requests, 0);
    let bad = [Operation::Relu { input: 1 }];
    assert!(program(&bad).layout().is_err());
}
#[test]
fn cancellation_deadline_and_clock_failure_clear_partial_results_without_refund() {
    let k = Double(false);
    let nodes = [Operation::Kernel {
        input: 0,
        kernel: &k,
    }];
    let p = program(&nodes);
    for reason in ["cancel", "deadline", "clock"] {
        let mut b = budget();
        if reason == "deadline" {
            b.deadline_tick = 3;
        }
        let mut s = Session::new(42, p.model_id, b).unwrap();
        let mut c = clock();
        if reason == "cancel" {
            c.cancel_at = Some(4);
        }
        if reason == "clock" {
            c.tick = 5;
            c.reverse = true;
        }
        let mut y = [9.; 3];
        let mut w = [9.; 2];
        let error = s
            .execute(42, &p, &[1., 2., 3.], &mut y, &mut w, &mut c)
            .unwrap_err();
        assert_eq!(
            error,
            match reason {
                "cancel" => Error::Cancelled,
                "deadline" => Error::Deadline,
                _ => Error::Clock,
            }
        );
        assert_eq!(y, [0.; 3]);
        assert_eq!(w, [0.; 2]);
        assert_eq!(s.usage().requests, 1);
        assert_eq!(s.usage().work_units, if reason == "clock" { 0 } else { 2 });
        assert_eq!(
            s.execute(42, &p, &[1., 2., 3.], &mut y, &mut w, &mut c),
            Err(Error::Closed)
        );
    }
}
#[test]
fn numeric_failure_is_terminal_only_for_its_own_session() {
    let good = Double(false);
    let bad = Double(true);
    let ok_nodes = [Operation::Kernel {
        input: 0,
        kernel: &good,
    }];
    let bad_nodes = [Operation::Kernel {
        input: 0,
        kernel: &bad,
    }];
    let p = program(&bad_nodes);
    let mut a = Session::new(1, p.model_id, budget()).unwrap();
    let mut b = Session::new(2, p.model_id, budget()).unwrap();
    let mut y = [9.];
    let mut w = [9.; 2];
    assert_eq!(
        a.execute(1, &p, &[1.], &mut y, &mut w, &mut clock()),
        Err(Error::Numeric)
    );
    assert_eq!(a.status(), Status::Failed);
    assert_eq!(a.usage().work_units, 2);
    assert_eq!(y, [0.]);
    assert_eq!(w, [0.; 2]);
    b.execute(2, &program(&ok_nodes), &[3.], &mut y, &mut w, &mut clock())
        .unwrap();
    assert_eq!(y, [6.]);
    assert_eq!(b.status(), Status::Ready);
}
