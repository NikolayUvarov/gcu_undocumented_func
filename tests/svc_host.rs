// svc's edits of data/services.txt (173-APP-0092): judged by init's own parser over the real boot order.
#![allow(dead_code)]
#[path = "../common/abi.rs"] mod abi;
#[path = "../init/src/services.rs"] mod services;
#[path = "../svc/src/plan.rs"] mod plan;

use abi::{BOOT_IMAGES, BOOT_SERVICES, SERVICE_INSTANCES};
use plan::Change;

const N: usize = BOOT_IMAGES + SERVICE_INSTANCES.len();

fn units() -> [&'static str; N] { plan::units(&BOOT_SERVICES, &SERVICE_INSTANCES) }

fn at(name: &str) -> usize { units().iter().position(|u| *u == name).unwrap() }

// The file after `change`, the other lines init refuses, or why the change is refused.
fn edit(text: &str, change: Change) -> Result<(String, plan::Edited<N>, Vec<usize>), &'static str> {
    let mut out = [0u8; services::MAX];
    let mut refused = Vec::new();
    let edited = plan::edit(text.as_bytes(), &units(), change, &mut out, |line, _, _| refused.push(line))?;
    Ok((String::from_utf8(out[..edited.len].to_vec()).unwrap(), edited, refused))
}

#[test]
fn units_are_inits_boot_order() {
    let units = units();
    assert_eq!(units[0], "init");
    assert_eq!(units[1], "logd");
    assert_eq!(units[N - 1], "shell");
    // Each instance follows its image.
    for instance in SERVICE_INSTANCES { let i = at(instance); assert!(units[..i].contains(&services::base(instance)), "{instance}"); }
    assert_eq!(units.iter().collect::<std::collections::HashSet<_>>().len(), N);
}

#[test]
fn disable_and_enable() {
    let (text, edited, refused) = edit("", Change::Disable("tts")).unwrap();
    assert_eq!(text, "disable tts\n");
    assert!(edited.changed && edited.plan.off[at("tts")] && refused.is_empty());
    // Again: the same file, nothing to write.
    let (again, edited, _) = edit(&text, Change::Disable("tts")).unwrap();
    assert_eq!(again, text);
    assert!(!edited.changed);
    // Enable takes the line out: the default is on.
    let (text, edited, _) = edit("# no speakers\ndisable tts\nafter windows sysmon\n", Change::Enable("tts")).unwrap();
    assert_eq!(text, "# no speakers\nafter windows sysmon\n");
    assert!(!edited.plan.off[at("tts")] && edited.plan.after[at("windows")] == Some(at("sysmon")));
    let (text, edited, _) = edit("", Change::Enable("tts")).unwrap();
    assert!(text.is_empty() && !edited.changed);
    // Every line for the name goes, whatever its spacing or comment; the others stay as they are.
    let (text, _, _) = edit("disable  tts  # loud\nenable tts\ndisable video_gw\n  # kept\n", Change::Disable("tts")).unwrap();
    assert_eq!(text, "disable video_gw\n  # kept\ndisable tts\n");
    // A file without its last newline gains one.
    let (text, _, _) = edit("disable video_gw", Change::Disable("tts")).unwrap();
    assert_eq!(text, "disable video_gw\ndisable tts\n");
}

#[test]
fn an_image_covers_its_instances() {
    let (_, edited, _) = edit("", Change::Disable("virtio_net")).unwrap();
    assert!(edited.plan.off[at("virtio_net")] && edited.plan.off[at("virtio_net#1")]);
    // `#` starts init's comment, so an instance cannot be named.
    assert!(edit("", Change::Disable("virtio_net#1")).err().unwrap().starts_with("AN INSTANCE"));
    assert!(edit("", Change::After("windows", "virtio_net#1")).err().unwrap().starts_with("AN INSTANCE"));
}

#[test]
fn what_needs_it_goes_off_too() {
    let (_, edited, _) = edit("", Change::Disable("keystore")).unwrap();
    assert_eq!(edited.plan.needs[at("tls")], Some(at("keystore")));
    assert_eq!(edited.plan.needs[at("ssh")], Some(at("keystore")));
    // Enabling tts keeps it off while audio_gw is.
    let (_, edited, _) = edit("disable audio_gw\ndisable tts\n", Change::Enable("tts")).unwrap();
    assert!(!edited.plan.off[at("tts")] && edited.plan.needs[at("tts")] == Some(at("audio_gw")));
}

#[test]
fn refused_as_init_refuses() {
    assert_eq!(edit("", Change::Disable("logd")).err(), Some("ESSENTIAL: INIT ALWAYS STARTS IT"));
    assert_eq!(edit("", Change::Disable("shell")).err(), Some("ESSENTIAL: INIT ALWAYS STARTS IT"));
    assert_eq!(edit("", Change::Enable("loader")).err(), Some("ESSENTIAL: INIT ALWAYS STARTS IT"));
    assert_eq!(edit("", Change::Disable("nosuchservice")).err(), Some("UNKNOWN SERVICE"));
    assert_eq!(edit("", Change::After("windows", "nosuchservice")).err(), Some("UNKNOWN SERVICE"));
    assert_eq!(edit("", Change::After("windows", "windows")).err(), Some("A SERVICE AFTER ITSELF"));
    // netstack starts after virtio_net by init's rules: virtio_net cannot wait for it.
    assert_eq!(edit("", Change::After("virtio_net", "netstack")).err(), Some("CONTRADICTS INIT: THE OTHER NEEDS IT FIRST"));
    // A cycle through the file's other line.
    assert_eq!(edit("after windows sysmon\n", Change::After("sysmon", "windows")).err(), Some("A CYCLE OF after"));
    assert_eq!(edit(&"#".repeat(services::MAX + 1), Change::Disable("tts")).err().map(|w| &w[..20]), Some("DATA/SERVICES.TXT IS"));
    assert_eq!(edit(&format!("{}\n", "#".repeat(services::MAX - 4)), Change::Disable("tts")).err(), Some("THE FILE WOULD PASS 4 KiB, WHAT INIT READS"));
}

#[test]
fn after_replaces_its_earlier_order() {
    let (text, edited, _) = edit("after windows sysmon\ndisable tts\n", Change::After("windows", "updater")).unwrap();
    assert_eq!(text, "disable tts\nafter windows updater\n");
    assert_eq!(edited.plan.after[at("windows")], Some(at("updater")));
}

#[test]
fn the_files_other_refused_lines_are_named() {
    let (text, _, refused) = edit("disable logd\nnot a directive\n", Change::Disable("tts")).unwrap();
    assert_eq!(text, "disable logd\nnot a directive\ndisable tts\n");
    assert_eq!(refused, vec![1, 2]);
}
