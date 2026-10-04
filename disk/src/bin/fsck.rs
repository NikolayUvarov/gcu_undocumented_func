#![no_std]
#![no_main]
// fsck [A:|ram:]: checks FAT volumes without changing them (vfs_server walks every chain from the directory tree and
// looks for lost and cross-linked clusters, broken chains, sizes that do not fit and invalid entries). A console
// program; without an argument it checks every volume.
use mind::abi::BootInfo;

mind::request!(REQUEST_CONSOLE);

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("fsck — checks FAT volumes without changing them: lost and cross-linked clusters, broken chains, sizes, entries.\nUsage: fsck [A:|ram:]   (without an argument: every volume)");
    let wanted = mind::process::args_str().trim().trim_end_matches('/').trim_end_matches(':');
    let volumes = [("", "A:"), ("ram", "ram:")];
    if !wanted.is_empty() && !volumes.iter().any(|(name, shown)| wanted.eq_ignore_ascii_case(name) || wanted.eq_ignore_ascii_case(shown.trim_end_matches(':'))) {
        mind::println!("fsck: no volume {} (A: or ram:)", wanted);
        return;
    }
    let mut damaged = 0;
    for (name, shown) in volumes {
        if !wanted.is_empty() && !wanted.eq_ignore_ascii_case(name) && !wanted.eq_ignore_ascii_case(shown.trim_end_matches(':')) { continue; }
        let label = mind::fs::volume(name).map(|v| (v.label, v.fat_bits));
        let result = mind::fs::check(name, |r| {
            if let Ok((label, bits)) = &label {
                mind::println!("{} {} FAT{}: {} files, {} directories; {} clusters used, {} free", shown, core::str::from_utf8(label).unwrap_or("").trim_end(), bits, r.files, r.directories, r.used, r.free);
            }
            let problems = r.lost + r.cross_linked + r.bad_chains + r.sizes + r.bad_entries;
            if problems == 0 { mind::println!("  clean"); } else {
                mind::println!("  ERRORS: lost clusters {} in {} chains, cross-linked {}, broken chains {}, size mismatches {}, bad entries {}", r.lost, r.lost_chains, r.cross_linked, r.bad_chains, r.sizes, r.bad_entries);
                mind::println!("  first: {}", r.first);
            }
            if r.dirty { mind::println!("  the volume is marked dirty: changed and not flushed yet"); }
            problems != 0
        });
        match result {
            Ok(true) => damaged += 1,
            Ok(false) => {}
            Err(error) => mind::println!("{} not checked ({:?})", shown, error),
        }
    }
    mind::println!("fsck: {} (nothing was changed)", if damaged == 0 { "no errors" } else if damaged == 1 { "1 volume with errors" } else { "volumes with errors" });
}
