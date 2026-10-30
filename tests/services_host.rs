// data/services.txt (173-KRN-0035): what init's parser turns off, orders and refuses, over the real boot order.
#![allow(dead_code)]
#[path = "../common/abi.rs"] mod abi;
#[path = "../init/src/services.rs"] mod services;
#[path = "../libmind/src/memconf.rs"] mod memconf;

use abi::{BOOT_IMAGES, BOOT_SERVICES, SERVICE_INSTANCES};
use services::*;

const N: usize = BOOT_IMAGES + SERVICE_INSTANCES.len();

// The units in init's boot order: init, then each image in start order (services::LATE moved) followed by its further
// instances.
fn units() -> [&'static str; N] {
    let mut units = ["init"; N];
    let mut p = 1;
    for image in start_images(&BOOT_SERVICES).map(|i| BOOT_SERVICES[i]) {
        units[p] = image; p += 1;
        for instance in SERVICE_INSTANCES.iter().filter(|i| base(i) == image) { units[p] = instance; p += 1; }
    }
    assert_eq!(p, N);
    units
}

fn at(units: &[&str; N], name: &str) -> usize { units.iter().position(|u| *u == name).unwrap_or_else(|| panic!("{name}")) }

fn parsed(text: &str) -> (Plan<N>, Vec<(usize, &'static str, String)>) {
    let mut refused = Vec::new();
    let plan = parse(text.as_bytes(), &units(), |line, why, text| refused.push((line, why, text.to_string())));
    (plan, refused)
}

// The order a boot takes: each position when ready, the waiting ones as soon as what they wait for was tried.
fn order(plan: &Plan<N>) -> Vec<&'static str> {
    let units = units();
    let (mut tried, mut waiting, mut out) = ([false; N], [false; N], Vec::new());
    tried[0] = true;
    for p in 1..N {
        if !plan.ready(&units, p, &tried) { waiting[p] = true; continue; }
        tried[p] = true; out.push(units[p]);
        while let Some(q) = (1..N).find(|&q| waiting[q] && plan.ready(&units, q, &tried)) { waiting[q] = false; tried[q] = true; out.push(units[q]); }
    }
    assert!(!waiting.iter().any(|&w| w), "every waiting unit started");
    out
}

#[test]
fn the_rules_name_real_services_in_their_boot_order() {
    let units = units();
    for name in CONFIGURABLE { at(&units, name); }
    for (who, need) in NEEDS.iter().chain(BEFORE.iter()) {
        assert!(at(&units, need) < at(&units, who), "{who} after {need} in BOOT_SERVICES");
    }
    // Everything before vfs_server is essential: init cannot read the file before it.
    for name in &units[..=at(&units, "vfs_server")] { assert!(!configurable(name), "{name}"); }
    for name in ["init", "logd", "compositor", "loader", "shell", "link", "effector_gw", "effector", "tpm"] { assert!(!configurable(name), "{name}"); }
}

#[test]
fn the_block_store_starts_right_after_keystore() {
    // 308-KRN-0183: its secrets client is keystore's; every image once, the others in BOOT_SERVICES' order.
    let units = units();
    assert_eq!(at(&units, "blockstore"), at(&units, "keystore") + 1);
    let rest: Vec<_> = units.iter().copied().filter(|u| !u.contains('#') && *u != "blockstore").collect();
    assert_eq!(rest, BOOT_SERVICES.iter().copied().filter(|u| *u != "blockstore").collect::<Vec<_>>());
    // A file cannot put keystore after it, and turning keystore off leaves the store on, without the client.
    let (plan, refused) = parsed("after keystore blockstore
");
    assert_eq!(refused, [(1, "CONTRADICTS INIT: THE OTHER NEEDS IT FIRST", "after keystore blockstore".to_string())]);
    assert_eq!(plan.after[at(&units, "keystore")], None);
    let (plan, _) = parsed("disable keystore
");
    assert!(!plan.off[at(&units, "blockstore")] && plan.needs[at(&units, "blockstore")].is_none());
    let order = order(&plan);
    assert!(order.iter().position(|u| *u == "keystore") < order.iter().position(|u| *u == "blockstore"));
}

#[test]
fn no_file_changes_nothing() {
    let (plan, refused) = parsed("");
    assert!(refused.is_empty());
    assert_eq!(plan.counts(), (0, 0, 0));
    assert_eq!(order(&plan), units()[1..].to_vec());
}

#[test]
fn the_maintainers_example_disables_and_orders() {
    let (plan, refused) = parsed("# the machine has no speakers\ndisable tts\ndisable video_gw   # nor a camera\n\nafter windows sysmon\n");
    assert!(refused.is_empty(), "{refused:?}");
    let units = units();
    assert!(plan.off[at(&units, "tts")] && plan.off[at(&units, "video_gw")]);
    assert_eq!(plan.counts(), (2, 0, 1));
    let order = order(&plan);
    let place = |name| order.iter().position(|u| *u == name).unwrap();
    assert!(place("sysmon") < place("windows") && place("windows") < place("updater"));
    assert_eq!(order.len(), N - 1);
}

#[test]
fn turning_one_off_turns_off_what_needs_it() {
    let units = units();
    let (plan, _) = parsed("disable netstack\n");
    for name in ["netpolicy", "tls", "ssh"] { assert_eq!(plan.needs[at(&units, name)], Some(at(&units, "netstack")), "{name}"); }
    assert_eq!(plan.needs[at(&units, "updater")], None); // it only starts after tls when tls runs
    let (plan, _) = parsed("disable keystore\n");
    assert_eq!(plan.needs[at(&units, "tls")], Some(at(&units, "keystore")));
    let (plan, _) = parsed("disable audio_gw\nenable audio_gw\n");
    assert_eq!(plan.counts(), (0, 0, 0)); // the later line wins
}

#[test]
fn an_image_names_its_instances() {
    let units = units();
    let (plan, refused) = parsed("disable virtio_net\nafter virtio_net sysmon\n");
    assert!(refused.is_empty(), "{refused:?}");
    for name in ["virtio_net", "virtio_net#1"] {
        assert!(plan.off[at(&units, name)]);
        assert_eq!(plan.after[at(&units, name)], Some(at(&units, "sysmon")));
    }
    // netstack starts after its cards, so after sysmon too.
    let order = order(&plan);
    let place = |name| order.iter().position(|u| *u == name).unwrap();
    assert!(place("sysmon") < place("virtio_net") && place("virtio_net") < place("virtio_net#1") && place("virtio_net#1") < place("netstack"));
}

#[test]
fn essential_unknown_and_malformed_lines_are_refused_and_change_nothing() {
    let (plan, refused) = parsed("disable logd\ndisable nosuchservice\nthis is not a directive\ndisable\nafter windows\n");
    let lines: Vec<_> = refused.iter().map(|(line, why, _)| (*line, *why)).collect();
    assert_eq!(lines, [(1, "ESSENTIAL: INIT ALWAYS STARTS IT"), (2, "UNKNOWN SERVICE"), (3, "NOT disable, enable OR after WITH ITS NAMES"),
                       (4, "NOT disable, enable OR after WITH ITS NAMES"), (5, "NOT disable, enable OR after WITH ITS NAMES")]);
    assert_eq!(refused[0].2, "disable logd");
    assert_eq!(plan.counts(), (0, 0, 0));
    let (_, refused) = parsed("disable shell\ndisable tpm\nafter shell windows\nafter windows nosuch\n");
    assert_eq!(refused.iter().map(|r| r.1).collect::<Vec<_>>(), ["ESSENTIAL: INIT ALWAYS STARTS IT", "ESSENTIAL: INIT ALWAYS STARTS IT",
                                                              "ESSENTIAL: INIT ALWAYS STARTS IT", "UNKNOWN SERVICE"]);
}

#[test]
fn an_order_against_inits_rules_or_in_a_cycle_is_refused() {
    let units = units();
    let (plan, refused) = parsed("after netstack tls\nafter keystore updater\nafter gpio shell\nafter tts tts\nafter windows sysmon\nafter sysmon windows\n");
    let why: Vec<_> = refused.iter().map(|(line, why, _)| (*line, *why)).collect();
    assert_eq!(why, [(1, "CONTRADICTS INIT: THE OTHER NEEDS IT FIRST"), (2, "CONTRADICTS INIT: THE OTHER NEEDS IT FIRST"),
                     (3, "CONTRADICTS INIT: THE OTHER NEEDS IT FIRST"), (4, "A SERVICE AFTER ITSELF"), (6, "A CYCLE OF after")]);
    assert_eq!(plan.after[at(&units, "windows")], Some(at(&units, "sysmon")));
    assert_eq!(plan.after[at(&units, "sysmon")], None);
    // A later `after` for the same service replaces the earlier one, and is checked without it.
    let (plan, refused) = parsed("after windows sysmon\nafter windows updater\nafter sysmon windows\nafter updater windows\n");
    assert_eq!(refused.iter().map(|r| (r.0, r.1)).collect::<Vec<_>>(), [(4, "A CYCLE OF after")]);
    assert_eq!(plan.after[at(&units, "windows")], Some(at(&units, "updater")));
    assert_eq!(plan.after[at(&units, "sysmon")], Some(at(&units, "windows")));
    let started = order(&plan);
    let place = |name| started.iter().position(|u| *u == name).unwrap();
    assert!(place("updater") < place("windows") && place("windows") < place("sysmon"));
}

#[test]
fn waiting_for_an_essential_service_keeps_inits_order() {
    // An essential service that starts later: what waits for it starts right after it, before the shell.
    let (plan, refused) = parsed("after tts effector\n");
    assert!(refused.is_empty(), "{refused:?}");
    let started = order(&plan);
    let place = |name| started.iter().position(|u| *u == name).unwrap();
    assert!(place("effector") < place("tts") && place("tts") < place("shell"));
    // One that already started: no change.
    let (plan, _) = parsed("after tts loader\n");
    assert_eq!(order(&plan), units()[1..].to_vec());
}

#[test]
fn a_service_its_dependencies_wait_for_holds_them_back() {
    // keystore waits for sysmon: tls and ssh, which need it, and the updater after tls, wait too; wlan as well.
    let (plan, refused) = parsed("after keystore sysmon\n");
    assert!(refused.is_empty(), "{refused:?}");
    let order = order(&plan);
    let place = |name| order.iter().position(|u| *u == name).unwrap();
    for name in ["tls", "ssh", "wlan", "updater"] { assert!(place("keystore") < place(name), "{name}"); }
    assert!(place("tls") < place("updater") && place("wlan") < place("netstack") && place("updater") < place("shell"));
}

#[test]
fn a_long_or_foreign_file_is_cut_or_skipped_by_line() {
    let mut text = "disable tts\n".to_string();
    text += &"# padding\n".repeat(500);
    text += "disable video_gw\n";
    let (plan, refused) = parsed(&text);
    assert_eq!(refused[0].1, "LONGER THAN 4 KiB: THE REST LEFT OUT");
    assert!(plan.off[at(&units(), "tts")] && !plan.off[at(&units(), "video_gw")]);
    let (plan, refused) = parsed("disable tts\ndisable vidéo\n\u{0}\u{1}\n");
    assert_eq!(refused.iter().map(|r| (r.0, r.1)).collect::<Vec<_>>(), [(2, "NOT ASCII"), (3, "NOT disable, enable OR after WITH ITS NAMES")]);
    assert_eq!(plan.counts(), (1, 0, 0));
    let (_, refused) = parsed("disable tts\r\nafter windows sysmon\r\n");
    assert!(refused.is_empty(), "{refused:?}"); // a file written on Windows
}

// data/memory.conf (262-KRN-0178): the line init reads for nnd at each start, as the loader reads a program's.
#[test]
fn the_memory_line_of_a_name() {
    let text = "# the owner's lines\nmemtest 160\nNND 64 # the model service\nnnd 32\nkbench\n";
    assert_eq!(memconf::mib(text, "nnd"), Some(64), "the first line of the name, its case aside");
    assert_eq!(memconf::mib(text, "memtest"), Some(160));
    assert_eq!(memconf::mib(text, "kbench"), None, "a line without its MiB gives nothing");
    assert_eq!(memconf::mib(text, "shell"), None);
    assert_eq!(memconf::mib("nnd 99999999\n", "nnd"), Some(65534), "at most one below the spawner's whole quota");
    assert_eq!(memconf::mib("# nnd 64\nnnd x\nnnd 8\n", "nnd"), Some(8), "a comment and a bad line are passed over");
}
