//! data/displays.txt's `output second off|read|light` line, which init passes to display at each start (213-KRN-0281,
//! for 213-DRV-0099).
#[path = "../libmind/src/displays.rs"] mod displays;
use displays::{second_output, Output};

#[test]
fn the_second_outputs_word() {
    let shell = "# The screens (177): how the first is shown on the second; screen second sets it\nsecond keep\n";
    assert_eq!(second_output(shell), Err(None), "the shell's lines alone name no output");
    assert_eq!(second_output(&format!("{shell}output second light\n")), Ok(Output::Light));
    assert_eq!(second_output("OUTPUT Second READ # the owner's\n"), Ok(Output::Read), "its case aside, a comment after");
    assert_eq!(second_output("output second off\r\n"), Ok(Output::Off), "a file written on Windows");
    assert_eq!(second_output("output second on\n"), Err(Some("on")));
    assert_eq!(second_output("output second light now\n"), Err(Some("light now")), "more than one word is another");
    assert_eq!(second_output("output second\n"), Err(Some("")));
    assert_eq!(second_output("# output second light\noutput second read\noutput second light\n"), Ok(Output::Read), "the first line, a comment passed over");
    assert_eq!(second_output(""), Err(None));
    assert_eq!(Output::Light.word(), "light");
    assert_eq!((displays::FILE, displays::MAX), ("data/displays.txt", 4096));
}
