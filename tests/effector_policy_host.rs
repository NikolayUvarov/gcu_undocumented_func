//! The Effector owner policy's grammar and decision table, and the gateway's audit line (750-EFF-0003).
#![allow(dead_code)]
#[path = "../effector_gw/src/policy.rs"] mod policy;
#[path = "../effector_gw/src/audit.rs"] mod audit;
use policy::*;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn full() -> Policy {
    let text = format!("# the owner's policy\neffector-policy 1\nagent on\nservice sysmon restart\nservice blockstore start,stop\nlog ring\ncollect data/reports/\ncollect log:\ndeliver data/inbox/\npackages on\npackage-key demo {KEY}\nupdate fetch\nreboot off\nscreen on\ntest files,log\nmax-bytes 1048576\n");
    Policy::parse(text.as_bytes()).expect("a valid policy")
}

fn kind(text: &str) -> (u16, ErrorKind) {
    let e = Policy::parse(text.as_bytes()).expect_err("must be refused");
    (e.line, e.kind)
}

#[test]
fn a_valid_policy_parses_and_counts_its_rules() {
    let p = full();
    assert!(p.agent());
    assert_eq!(p.rules(), 14);
    assert_eq!(p.max_bytes(), 1048576);
    assert_eq!(p.keys_for("demo").count(), 1);
    assert_eq!(p.keys_for("other").count(), 0);
}

#[test]
fn only_the_version_line_gives_an_agent_that_is_off() {
    let p = Policy::parse(b"effector-policy 1\n").unwrap();
    assert!(!p.agent());
    assert_eq!(p.decide(Ask::Log("ring")), Err(Deny::AgentOff));
    assert_eq!(Policy::default().decide(Ask::Reboot), Err(Deny::AgentOff), "no policy denies everything");
}

#[test]
fn comments_blank_lines_crlf_and_tabs_are_accepted() {
    let p = Policy::parse(b"# a comment\r\n\r\neffector-policy 1\r\n\tagent\ton  \r\n# \xd0\xba\xd0\xbe\xd0\xbc\xd0\xbc\xd0\xb5\xd0\xbd\xd1\x82\n").unwrap();
    assert!(p.agent());
}

#[test]
fn an_unknown_key_refuses_the_whole_file() {
    assert_eq!(kind("effector-policy 1\nagent on\nlimit-bytes 5\n"), (3, ErrorKind::UnknownKey));
    assert_eq!(kind("effector-policy 1\nAgent on\n"), (2, ErrorKind::UnknownKey), "keys are case-sensitive");
}

#[test]
fn the_version_line_comes_first_and_is_known() {
    assert_eq!(kind("agent on\neffector-policy 1\n"), (1, ErrorKind::Version));
    assert_eq!(kind("effector-policy 2\nagent on\n"), (1, ErrorKind::Version));
    assert_eq!(kind("# only a comment\n"), (0, ErrorKind::Version));
    assert_eq!(kind(""), (0, ErrorKind::Version));
}

#[test]
fn a_file_too_large_or_not_text_is_refused() {
    let mut big = String::from("effector-policy 1\n");
    while big.len() <= FILE_MAX { big.push_str("# padding padding padding padding\n"); }
    assert_eq!(kind(&big), (0, ErrorKind::TooLarge));
    assert_eq!(Policy::parse(b"effector-policy 1\n\xff\n").unwrap_err().kind, ErrorKind::NotText);
    assert_eq!(kind("effector-policy 1\nagent on\x07\n"), (2, ErrorKind::NotText));
    let long = format!("effector-policy 1\n# {}\n", "x".repeat(LINE_MAX));
    assert_eq!(kind(&long), (2, ErrorKind::LineTooLong));
}

#[test]
fn too_many_rules_or_items_are_refused() {
    let mut many = String::from("effector-policy 1\n");
    for i in 0..=RULES_MAX { many.push_str(&format!("service s{i} start\n")); }
    let e = Policy::parse(many.as_bytes()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::TooMany, "{e:?}");
    let mut roots = String::from("effector-policy 1\n");
    for i in 0..=ROOTS_MAX { roots.push_str(&format!("collect data/r{i}/\n")); }
    assert_eq!(Policy::parse(roots.as_bytes()).unwrap_err().kind, ErrorKind::TooMany);
}

#[test]
fn malformed_and_duplicate_lines_are_refused() {
    for (text, line, k) in [
        ("effector-policy 1\nagent yes\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nagent on off\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nagent\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nagent on\nagent off\n", 3, ErrorKind::Duplicate),
        ("effector-policy 1\nservice sysmon reload\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nservice SysMon start\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nservice sysmon start\nservice sysmon stop\n", 3, ErrorKind::Duplicate),
        ("effector-policy 1\nlog kernel\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ncollect data/reports\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ncollect data/../x/\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ncollect ram:/\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ncollect /data/x/\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ndeliver data/\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ndeliver log:/x/\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ndeliver data/a/\ndeliver data/b/\n", 3, ErrorKind::Duplicate),
        ("effector-policy 1\npackage-key demo 0123\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\npackage-key Demo 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nupdate always\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ntest network,sockets\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\ntest\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nmax-bytes 0\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nmax-bytes 01\n", 2, ErrorKind::Malformed),
        ("effector-policy 1\nmax-bytes 1073741825\n", 2, ErrorKind::Malformed),
    ] {
        assert_eq!(kind(text), (line, k), "{text:?}");
    }
}

#[test]
fn protected_services_trees_and_grants_cannot_be_named() {
    for name in ["init", "shell", "effector", "effector_gw"] {
        assert_eq!(kind(&format!("effector-policy 1\nservice {name} restart\n")), (2, ErrorKind::Protected), "{name}");
    }
    for root in ["data/effector/", "data/apps/", "data/effector/sub/"] {
        assert_eq!(kind(&format!("effector-policy 1\ncollect {root}\n")), (2, ErrorKind::Protected), "{root}");
        assert_eq!(kind(&format!("effector-policy 1\ndeliver {root}\n")), (2, ErrorKind::Protected), "{root}");
    }
    for grant in ["firmware", "authority", "tls", "ssh"] {
        assert_eq!(kind(&format!("effector-policy 1\ntest files,{grant}\n")), (2, ErrorKind::Protected), "{grant}");
    }
}

#[test]
fn the_decision_table() {
    let p = full();
    // Services: only the named action of a named service.
    assert_eq!(p.decide(Ask::Service("sysmon", Action::Restart)), Ok(()));
    assert_eq!(p.decide(Ask::Service("sysmon", Action::Stop)), Err(Deny::NotNamed));
    assert_eq!(p.decide(Ask::Service("blockstore", Action::Stop)), Ok(()));
    assert_eq!(p.decide(Ask::Service("rtc", Action::Start)), Err(Deny::NotNamed));
    assert_eq!(p.decide(Ask::Service("shell", Action::Restart)), Err(Deny::Protected));
    // Logs.
    assert_eq!(p.decide(Ask::Log("ring")), Ok(()));
    assert_eq!(p.decide(Ask::Log("boot")), Err(Deny::NotNamed));
    // Files out: under a root, within the size, never a protected tree.
    assert_eq!(p.decide(Ask::Collect("data/reports/a.txt", 10)), Ok(()));
    assert_eq!(p.decide(Ask::Collect("data/reports/deep/b.bin", 1048576)), Ok(()));
    assert_eq!(p.decide(Ask::Collect("data/reports/a.txt", 1048577)), Err(Deny::TooLarge));
    assert_eq!(p.decide(Ask::Collect("data/reportsx/a.txt", 1)), Err(Deny::NotNamed), "a root is a whole directory name");
    assert_eq!(p.decide(Ask::Collect("data/reports/", 1)), Err(Deny::Path));
    assert_eq!(p.decide(Ask::Collect("data/reports/../effector/token", 1)), Err(Deny::Path));
    assert_eq!(p.decide(Ask::Collect("log:/boot0003.log", 1)), Ok(()));
    assert_eq!(p.decide(Ask::Collect("data/other.txt", 1)), Err(Deny::NotNamed));
    assert_eq!(p.decide(Ask::Collect("data/effector/audit", 1)), Err(Deny::Protected));
    // Files in: under the delivery root only.
    assert_eq!(p.decide(Ask::Deliver("data/inbox/x.bin", 1)), Ok(()));
    assert_eq!(p.decide(Ask::Deliver("data/reports/x.bin", 1)), Err(Deny::NotNamed));
    assert_eq!(p.decide(Ask::Deliver("data/apps/demo/1/demo.elf", 1)), Err(Deny::Protected));
    // Packages: on, and a key trusted for the name.
    assert_eq!(p.decide(Ask::Package("demo", 100)), Ok(()));
    assert_eq!(p.decide(Ask::Package("other", 100)), Err(Deny::NotNamed));
    assert_eq!(p.decide(Ask::Package("demo", 2 << 20)), Err(Deny::TooLarge));
    // Updates: up to the named step.
    assert_eq!(p.decide(Ask::Update(Update::Check)), Ok(()));
    assert_eq!(p.decide(Ask::Update(Update::Fetch)), Ok(()));
    assert_eq!(p.decide(Ask::Update(Update::Apply)), Err(Deny::NotNamed));
    assert_eq!(p.decide(Ask::Update(Update::None)), Err(Deny::NotNamed));
    // Switches.
    assert_eq!(p.decide(Ask::Reboot), Err(Deny::NotNamed));
    assert_eq!(p.decide(Ask::Screen), Ok(()));
    // Tests: only grants the policy lists.
    assert_eq!(p.decide(Ask::Test(&["files"])), Ok(()));
    assert_eq!(p.decide(Ask::Test(&[])), Ok(()));
    assert_eq!(p.decide(Ask::Test(&["files", "network"])), Err(Deny::NotNamed));
}

#[test]
fn not_named_means_denied() {
    let p = Policy::parse(b"effector-policy 1\nagent on\n").unwrap();
    let asks = [
        Ask::Service("sysmon", Action::Restart), Ask::Log("ring"), Ask::Log("boot"),
        Ask::Collect("data/x.txt", 1), Ask::Collect("log:/boot0001.log", 1), Ask::Deliver("data/inbox/x", 1),
        Ask::Package("demo", 1), Ask::Update(Update::Check), Ask::Reboot, Ask::Screen, Ask::Test(&[]),
    ];
    for ask in asks { assert_eq!(p.decide(ask), Err(Deny::NotNamed), "{ask:?}"); }
    let packages_without_key = Policy::parse(b"effector-policy 1\nagent on\npackages on\n").unwrap();
    assert_eq!(packages_without_key.decide(Ask::Package("demo", 1)), Err(Deny::NotNamed));
    let key_without_packages = Policy::parse(format!("effector-policy 1\nagent on\npackage-key * {KEY}\n").as_bytes()).unwrap();
    assert_eq!(key_without_packages.decide(Ask::Package("demo", 1)), Err(Deny::NotNamed));
}

#[test]
fn the_audit_line_is_bounded_and_printable() {
    let e = audit::Entry { uptime_ms: 123456, command: "20261021064900.000000001_000042", request: "service-control sysmon restart", decision: "allow", result: "completed", initiator: "operator \"x\"", reason: "restart after the update\nwith a newline" };
    let l = audit::format(&e);
    assert_eq!(l.as_str(), "EFFECTOR 123456 20261021064900.000000001_000042 service-control?sysmon?restart allow completed by=\"operator ?x?\" why=\"restart after the update?with a newline\"");
    assert!(!l.cut);
    let long = "\u{0436}".repeat(400);
    let e = audit::Entry { reason: &long, ..e };
    let l = audit::format(&e);
    assert_eq!(l.as_bytes().len(), audit::LINE_MAX);
    assert!(l.cut && l.as_str().ends_with("~\""), "{}", l.as_str());
    assert!(l.as_bytes().iter().all(|b| (0x20..0x7f).contains(b)));
    let empty = audit::Entry { command: "", initiator: "", reason: "", ..e };
    assert!(audit::format(&empty).as_str().starts_with("EFFECTOR 123456 - "));
}
