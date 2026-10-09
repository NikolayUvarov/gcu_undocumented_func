//! Host tests of the JSON reader (libmind/src/json.rs) and the model manifest reader (libmind/src/models.rs), 251-STO-0014:
//! documents Python's json writes are read through and their strings decode as Python reads them; malformed ones are
//! refused; the manifest of models/manifest.toml, as scripts/models.py writes it on a model disk, reads model by model
//! with every file's size and SHA-256 and each entry's own bytes.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/json.rs"]
mod json;
#[path = "../libmind/src/models.rs"]
mod models;

use json::{Kind, Reader};
use std::process::Command;

fn python(code: &str) -> Vec<u8> {
    let out = Command::new("python3").args(["-c", &format!("import sys, json; sys.path.insert(0, 'scripts')\n{}", code)]).output().expect("python3");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

// Every string of a document, in order, as the reader decodes them.
fn strings(text: &[u8]) -> Vec<String> {
    fn walk(r: &mut Reader, out: &mut Vec<String>) {
        let mut buf = [0u8; 4096];
        match r.peek().unwrap() {
            Kind::Object => { r.object().unwrap(); while let Some(k) = r.key(&mut buf).unwrap() { out.push(String::from_utf8(k.to_vec()).unwrap()); walk(r, out); } }
            Kind::Array => { r.array().unwrap(); while r.item().unwrap() { walk(r, out); } }
            Kind::String => out.push(String::from_utf8(r.string(&mut buf).unwrap().to_vec()).unwrap()),
            _ => r.skip().unwrap(),
        }
    }
    let mut r = Reader::new(text).unwrap();
    let mut out = Vec::new();
    walk(&mut r, &mut out);
    r.end().unwrap();
    out
}

#[test]
fn documents_python_writes_read_as_python_reads_them() {
    let doc = r#"{"a": [1, -2.5e+3, 0, true, false, null, {"b": "x\"y\\z\/\b\f\n\r\t"}], "Ünï": "\u00e9\ud83d\ude00 голос", "": [], "e": {}}"#;
    // Python's own encodings of the same value: ASCII-escaped, indented, and compact.
    for code in ["json.dumps(v)", "json.dumps(v, ensure_ascii=False, indent=1)", "json.dumps(v, separators=(',', ':'))"] {
        let text = python(&format!("v = json.loads({:?})\nsys.stdout.buffer.write({}.encode())", doc, code));
        let expected: Vec<String> = serde_like(&python(&format!("v = json.loads({:?})\nout = []\n\
            def walk(x):\n    if isinstance(x, dict):\n        [ (out.append(k), walk(w)) for k, w in x.items() ]\n    elif isinstance(x, list):\n        [walk(w) for w in x]\n    elif isinstance(x, str):\n        out.append(x)\n\
            walk(v)\nsys.stdout.buffer.write(json.dumps(out).encode())", doc)));
        assert_eq!(strings(&text), expected, "{}", code);
    }
    let mut r = Reader::new(b" 18446744073709551615 ").unwrap();
    assert_eq!(r.integer(), Ok(u64::MAX));
}

// A JSON list of strings, read with the reader under test only after the other tests trust it for flat lists.
fn serde_like(text: &[u8]) -> Vec<String> {
    let mut r = Reader::new(text).unwrap();
    let mut buf = [0u8; 4096];
    let mut out = Vec::new();
    r.array().unwrap();
    while r.item().unwrap() { out.push(String::from_utf8(r.string(&mut buf).unwrap().to_vec()).unwrap()); }
    out
}

#[test]
fn malformed_documents_are_refused() {
    let deep = format!("{}{}", "[".repeat(json::DEPTH + 1), "]".repeat(json::DEPTH + 1));
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("a trailing comma", b"[1,]".to_vec()), ("a trailing comma in an object", br#"{"a":1,}"#.to_vec()),
        ("a leading zero", b"[01]".to_vec()), ("a bare minus", b"[-]".to_vec()), ("a dot without digits", b"[1.]".to_vec()),
        ("an exponent without digits", b"[1e]".to_vec()), ("a plus sign", b"[+1]".to_vec()),
        ("a control character in a string", b"[\"a\tb\"]".to_vec()), ("a lone high surrogate", br#"["\ud83d"]"#.to_vec()),
        ("a lone low surrogate", br#"["\ude00"]"#.to_vec()), ("an unknown escape", br#"["\x41"]"#.to_vec()),
        ("an unterminated string", br#"["abc]"#.to_vec()), ("an unclosed array", b"[1, 2".to_vec()),
        ("a missing colon", br#"{"a" 1}"#.to_vec()), ("a key that is not a string", b"{a: 1}".to_vec()),
        ("single quotes", b"['a']".to_vec()), ("a comment", b"[1 /* x */]".to_vec()), ("two values", b"[1] [2]".to_vec()),
        ("invalid UTF-8", b"[\"\xff\"]".to_vec()), ("TRUE", b"[TRUE]".to_vec()), ("nesting too deep", deep.into_bytes()),
        ("missing commas", b"[1 2]".to_vec()),
    ];
    for (what, text) in cases {
        let refused = Reader::new(&text).and_then(|mut r| { r.skip()?; r.end() }).is_err();
        assert!(refused, "{}: {}", what, String::from_utf8_lossy(&text));
    }
    let ok = format!("{}{}", "[".repeat(json::DEPTH), "]".repeat(json::DEPTH));
    assert!(Reader::new(ok.as_bytes()).and_then(|mut r| { r.skip()?; r.end() }).is_ok());
    // A non-negative integer only.
    for text in ["-1", "1.0", "1e3", "18446744073709551616"] { assert!(Reader::new(text.as_bytes()).unwrap().integer().is_err(), "{}", text); }
}

// models/manifest.toml as scripts/models.py writes MANIFEST.json on a model disk.
fn disk_manifest() -> Vec<u8> {
    python("import tomllib\nm = tomllib.load(open('models/manifest.toml', 'rb'))['model']\n\
            sys.stdout.buffer.write(json.dumps({'format': 1, 'models': m}, ensure_ascii=False, indent=1).encode())")
}

#[test]
fn the_model_disk_manifest_reads_model_by_model() {
    let text = disk_manifest();
    let expected: Vec<(String, Vec<(String, u64, String)>)> = {
        let out = String::from_utf8(python("import tomllib\nm = tomllib.load(open('models/manifest.toml', 'rb'))['model']\n\
            for x in m:\n    print(x['id'])\n    for f in x['files']: print(' ', f['path'], f['size'], f['sha256'])")).unwrap();
        let mut models: Vec<(String, Vec<(String, u64, String)>)> = Vec::new();
        for line in out.lines() {
            if let Some(f) = line.strip_prefix("  ") {
                let p: Vec<&str> = f.split(' ').collect();
                models.last_mut().unwrap().1.push((p[0].into(), p[1].parse().unwrap(), p[2].into()));
            } else { models.push((line.into(), Vec::new())); }
        }
        models
    };
    for (i, (id, files)) in expected.iter().enumerate() {
        // Files a page of 8 at a time, as the parser service gives them.
        let mut got = Vec::new();
        let mut first = 0;
        loop {
            let mut page = [models::File::EMPTY; 8];
            let (count, model, n) = models::read(&text, i, first, &mut page).unwrap();
            assert_eq!((count, model.id.as_str(), model.files), (expected.len(), id.as_str(), files.len()));
            got.extend(page[..n].iter().map(|f| (f.path.as_str().to_string(), f.size, f.sha256.iter().map(|b| format!("{:02x}", b)).collect::<String>())));
            first += n;
            if n < 8 { break; }
        }
        assert_eq!(&got, files, "{}", id);
        // The entry's own bytes are the model's whole object: Python reads them as the same model.
        let (_, model, _) = models::read(&text, i, 0, &mut []).unwrap();
        let entry = std::str::from_utf8(&text[model.start..model.end]).unwrap();
        let same = python(&format!("import tomllib\nm = tomllib.load(open('models/manifest.toml', 'rb'))['model']\nprint(json.loads({:?}) == m[{}])", entry, i));
        assert_eq!(same, b"True\n", "{}", id);
    }
    assert!(models::read(&text, expected.len(), 0, &mut []).is_err(), "no model past the last");
}

#[test]
fn other_manifests_are_refused() {
    let good = br#"{"format": 1, "models": [{"id": "a", "files": [{"path": "x/y.bin", "size": 3, "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}]}]}"#;
    assert!(models::read(good, 0, 0, &mut []).is_ok());
    let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let changed = |from: &str, to: &str| String::from_utf8(good.to_vec()).unwrap().replacen(from, to, 1).into_bytes();
    let twice = String::from_utf8(good.to_vec()).unwrap().replace("}]}]}", "}]}, {\"id\": \"a\", \"files\": []}]}").into_bytes();
    let path_twice = changed("}]}]}", &format!("}}, {{\"path\": \"x/y.bin\", \"size\": 1, \"sha256\": \"{}\"}}]}}]}}", digest));
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("format 2", changed("\"format\": 1", "\"format\": 2")), ("no models", changed("\"models\"", "\"modelz\"")),
        ("a model without files", changed("\"files\"", "\"filez\"")), ("a model without an id", changed("\"id\"", "\"ident\"")),
        ("an id with a slash", changed("\"a\"", "\"a/b\"")), ("a path with ..", changed("x/y.bin", "../y.bin")),
        ("an absolute path", changed("x/y.bin", "/y.bin")), ("a path with an empty part", changed("x/y.bin", "x//y.bin")),
        ("an upper-case digest", changed(digest, &digest.to_uppercase())), ("a short digest", changed(digest, &digest[2..])),
        ("a negative size", changed("\"size\": 3", "\"size\": -3")), ("a size with a fraction", changed("\"size\": 3", "\"size\": 3.0")),
        ("an id twice", twice), ("a path twice in a model", path_twice), ("not JSON", b"format: 1".to_vec()),
    ];
    for (what, text) in cases { assert!(models::read(&text, 0, 0, &mut []).is_err(), "{}: {}", what, String::from_utf8_lossy(&text)); }
    // Without a format it is read as format 1, as the QEMU suites' empty model disks write it.
    assert_eq!(models::read(br#"{"models": [{"id": "z", "files": []}]}"#, 0, 0, &mut []).map(|(n, m, _)| (n, m.files)), Ok((1, 0)));
}
