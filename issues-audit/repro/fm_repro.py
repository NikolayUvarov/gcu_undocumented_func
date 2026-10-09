"""Exercise real fm code using its existing host test doubles, in a temporary file."""
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / 'tests/fm_host.rs').read_text()
source = re.sub(r'#\[path = "\.\./([^\"]+)"\]', lambda m: '#[path = "' + str(ROOT / m[1]) + '"]', source)
# Observe when the existing mock receives deletion, without changing its behavior.
anchor = '    fn remove(&mut self, path: &str) -> Result<(), Failure> {\n'
assert source.count(anchor) == 1
source = source.replace(anchor, anchor + '''        if path == "data/audit-source.txt" {
            self.runs.push(format!("source deletion: flushes={}", self.flushes));
        }
''')
source += '''

#[test]
fn audit_save_destroys_an_existing_temporary_name() {
    let mut disk = Mem::sample();
    disk.dirs.push("ram:notes".into());
    disk.add_file("ram:notes/todo.txt", b"original");
    disk.add_file("ram:notes/todo.txt.tmp", b"unrelated valuable file");
    let mut window=vec![0u8;4096];
    let mut fm=Fm::new(&mut window,&mut disk);
    fm.load(0,"ram:notes",Some("todo.txt"),&mut disk);
    fm.key(f(4),&mut disk);
    typed(&mut fm,&mut disk,"new ");
    fm.key(f(2),&mut disk);
    assert_eq!(disk.file("ram:notes/todo.txt").unwrap(),b"new original");
    assert!(disk.file("ram:notes/todo.txt.tmp").is_none());
    println!("SAVE: unrelated todo.txt.tmp disappeared after F2");
}

#[test]
fn audit_move_removes_source_before_any_destination_flush() {
    let mut disk = Mem::sample();
    disk.dirs.push("data".into());
    disk.add_file("data/audit-source.txt",b"original");
    let mut window=vec![0u8;4096];
    let mut fm=Fm::new(&mut window,&mut disk);
    ram_panel(&mut fm,&mut disk);
    fm.load(0,"data",Some("audit-source.txt"),&mut disk);
    fm.key(f(6),&mut disk);
    fm.key(code(KEY_ENTER),&mut disk);
    run(&mut fm,&mut disk);
    assert!(disk.file("data/audit-source.txt").is_none());
    assert_eq!(disk.file("ram:audit-source.txt").unwrap(),b"original");
    assert!(disk.runs.iter().any(|s|s=="source deletion: flushes=0"));
    println!("MOVE: {}",disk.runs.join(", "));
}
'''
with tempfile.TemporaryDirectory(prefix='mind-audit-fm-') as temporary:
    case=Path(temporary)/'fm_repro.rs'
    case.write_text(source)
    binary=Path(temporary)/'fm_repro'
    subprocess.run(['rustc','--edition=2021','--test',str(case),'-o',str(binary)],check=True,cwd=ROOT)
    subprocess.run([str(binary),'audit_','--nocapture'],check=True,cwd=ROOT)
