//! Host tests of `get`'s words and options (get/src/args.rs, 000-NET-0048): quoting as a shell does it, every option,
//! what is refused, the method chosen, the name -O saves to, and the exit statuses curl's scripts read.
extern crate alloc;
#[path = "../get/src/args.rs"]
mod args;

use args::{parse, remote_name, split, Data, Form, Output};

fn words(text: &str) -> Vec<String> { split(text).unwrap() }

#[test]
fn words_split_as_a_shell_splits_them() {
    assert_eq!(words("get  -H 'X-Token: a b' -d \"{\\\"k\\\": 1}\" https://h/p"), ["get", "-H", "X-Token: a b", "-d", "{\"k\": 1}", "https://h/p"]);
    assert_eq!(words("a\\ b '' c"), ["a b", "", "c"]);
    assert_eq!(words("  "), Vec::<String>::new());
    assert!(split("'open").is_err() && split("\"open").is_err() && split("\"a\\").is_err());
}

#[test]
fn every_option() {
    let o = parse(&words("-o page.html -c -H 'Accept: text/html' -H X-A:1 --max-redirs 3 -s -w -f -m 30 https://api.test/x")).unwrap();
    assert_eq!(o.url, "https://api.test/x");
    assert_eq!(o.output, Output::File("page.html".into()));
    assert!(o.resume && o.silent && o.write_out && o.fail && o.follow && !o.head && !o.insecure_http);
    assert_eq!(o.headers, [("Accept".to_string(), "text/html".to_string()), ("X-A".to_string(), "1".to_string())]);
    assert_eq!((o.max_redirs, o.max_time, o.method()), (3, Some(30), "GET"));
    let o = parse(&words("-d @body.json -X put --no-location --insecure-http http://h/")).unwrap();
    assert_eq!((o.data.clone(), o.method(), o.follow, o.insecure_http), (Some(Data::File("body.json".into())), "PUT", false, true));
    let o = parse(&words("-F chat_id=7 -F 'doc=@ram:a b.pdf' -O https://h/files/r.pdf?x=1")).unwrap();
    assert_eq!(o.form, [Form::Field { name: "chat_id".into(), value: "7".into() }, Form::File { name: "doc".into(), path: "ram:a b.pdf".into() }]);
    assert_eq!((o.output.clone(), o.method()), (Output::Remote, "POST"));
    assert_eq!(parse(&words("-d hello https://h/")).unwrap().data, Some(Data::Text("hello".into())));
    assert_eq!(parse(&words("-d @ https://h/")).unwrap().data, Some(Data::Text("@".into())));
    let o = parse(&words("-I https://h/")).unwrap();
    assert_eq!((o.head, o.method()), (true, "HEAD"));
    // HEAD, PATCH and DELETE by -X; -X HEAD shows the head as -I does (000-NET-0047).
    let o = parse(&words("-X head https://h/")).unwrap();
    assert_eq!((o.head, o.method()), (true, "HEAD"));
    assert_eq!(parse(&words("-X PATCH -d {} https://h/")).unwrap().method(), "PATCH");
    let o = parse(&words("-X DELETE https://h/")).unwrap();
    assert_eq!((o.head, o.method()), (false, "DELETE"));
}

#[test]
fn what_is_refused() {
    for bad in ["", "-o", "-x https://h/", "https://a/ https://b/", "-X TRACE https://h/", "-X HEAD -d a https://h/", "-H nocolon https://h/", "-H 'bad name: v' https://h/",
                "-d a -F b=c https://h/", "-I -d a https://h/", "-c https://h/", "--max-redirs many https://h/", "--max-redirs 11 https://h/", "-m 0 https://h/", "-F =v https://h/",
                "-F novalue https://h/"] {
        assert!(parse(&words(bad)).is_err(), "{:?}", bad);
    }
    let many = (0..15).map(|i| format!("-H 'H{}: v'", i)).collect::<Vec<_>>().join(" ");
    assert!(parse(&words(&format!("{} https://h/", many))).is_err());
}

#[test]
fn the_name_minus_o_saves_to() {
    assert_eq!(remote_name("/files/report.pdf?x=1#top"), Some("report.pdf"));
    assert_eq!(remote_name("/a/b"), Some("b"));
    for none in ["/", "/dir/", "/..", "/a/.", "/a:b", "/a*b", ""] { assert_eq!(remote_name(none), None, "{:?}", none); }
}

#[test]
fn exit_statuses_are_curls() {
    assert_eq!([args::OK, args::UNSUPPORTED, args::USAGE, args::BAD_URL, args::NO_HOST, args::NO_CONNECTION, args::HTTP_ERROR, args::WRITE_ERROR,
                args::READ_ERROR, args::TIMEOUT, args::TLS_ERROR, args::TOO_MANY_REDIRECTS, args::RECEIVE_ERROR, args::CERTIFICATE],
               [0, 1, 2, 3, 6, 7, 22, 23, 26, 28, 35, 47, 56, 60]);
}
