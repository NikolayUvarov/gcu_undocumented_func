//! Host tests of the window manager's desktop (wm/src/desk.rs, issue 088): where new windows go, focus and z-order,
//! halves, quarters, maximize, moving, resizing and snapping, what the mouse hits and drags, the keys wm keeps and
//! the ones it passes on, and what each cell of the screen shows (text content, frames, pixel windows below others).
#![allow(dead_code)]
extern crate alloc;
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;
#[path = "../wm/src/desk.rs"]
mod desk;

use abi::*;
use desk::{Action, Content, Desk, Hit, Win, Wm};
use keys::{event, Key};
use tui::{Cell, Grid, Rect, DARK};

fn text(id: u32, title: &str) -> Win { Win::new(id, 100 + id as u64, Content::Text, (80, 25), title) }
fn pixels(id: u32, title: &str, w: usize, h: usize) -> Win { Win::new(id, 100 + id as u64, Content::Pixels, (w, h), title) }
fn rect(desk: &Desk, id: u32) -> (usize, usize, usize, usize) { let r = desk.get(id).unwrap().rect; (r.x, r.y, r.w, r.h) }
fn alt(code: u16, ch: char) -> Key { Key(event(code, ch as u32, MOD_ALT)) }
fn alt_char(ch: char) -> Key { alt(0, ch) }
fn key(code: u16) -> Key { Key(event(code, 0, 0)) }
fn chr(ch: char) -> Key { Key(event(0, ch as u32, 0)) }

// The desktop of a 1280 × 800 screen: 160 × 50 cells, windows between the top bar and the status line.
fn desk_of_four() -> Desk {
    let mut desk = Desk::new(160, 50);
    desk.add(text(1, "fm"));
    desk.add(text(2, "fm"));
    desk.add(pixels(3, "clock", 320, 176));
    desk.add(pixels(4, "dzen-clock", 400, 320));
    desk
}

#[test]
fn new_windows_take_the_quarters_then_cascade() {
    let mut desk = desk_of_four();
    assert_eq!(desk.area(), Rect::new(0, 1, 160, 48));
    assert_eq!(rect(&desk, 1), (0, 1, 80, 24), "a text window fills its quarter");
    assert_eq!(rect(&desk, 2), (80, 1, 80, 24));
    assert_eq!(rect(&desk, 3), (0, 25, 42, 13), "a pixel window gets the frame its pixels need");
    assert_eq!(rect(&desk, 4), (80, 25, 52, 22));
    desk.add(text(5, "top"));
    assert_eq!(rect(&desk, 5), (0, 1, 82, 27), "the fifth cascades from the top left");
    desk.add(text(6, "edit"));
    assert_eq!(rect(&desk, 6), (2, 3, 82, 27));
    assert_eq!(desk.focus(), Some(6), "a new window has the focus");
    // A saved place is kept (only moved inside the screen).
    let mut saved = text(7, "load");
    saved.rect = Rect::new(150, 45, 40, 10);
    desk.add(saved);
    assert_eq!(rect(&desk, 7), (120, 39, 40, 10));
    // A small screen: pixel windows are cut to their quarter.
    let mut small = Desk::new(80, 25);
    small.add(pixels(1, "dzen-clock", 400, 320));
    assert_eq!(rect(&small, 1), (0, 1, 40, 11));
}

#[test]
fn focus_z_order_and_removal() {
    let mut desk = desk_of_four();
    assert_eq!(desk.focus(), Some(4));
    desk.raise(1);
    assert_eq!((desk.focus(), desk.windows.iter().map(|w| w.id).collect::<Vec<_>>()), (Some(1), vec![2, 3, 4, 1]));
    desk.cycle(false);
    assert_eq!(desk.focus(), Some(2), "Alt+Tab brings the bottom window to the front");
    desk.cycle(true);
    assert_eq!(desk.focus(), Some(1), "Alt+Shift+Tab sends the front one to the bottom");
    desk.remove(1);
    assert_eq!(desk.focus(), Some(4));
    assert!(desk.status().starts_with("FOCUS=4 WINDOWS=2@80,1,80x24 3@0,25,42x13 4@80,25,52x22"), "{}", desk.status());
}

#[test]
fn halves_quarters_maximize_and_snapping() {
    let mut desk = desk_of_four();
    desk.half(1, 0);
    assert_eq!(rect(&desk, 1), (0, 1, 80, 48));
    desk.half(1, 1);
    assert_eq!(rect(&desk, 1), (80, 1, 80, 48));
    desk.half(1, 2);
    assert_eq!(rect(&desk, 1), (0, 1, 160, 24));
    desk.half(1, 3);
    assert_eq!(rect(&desk, 1), (0, 25, 160, 24));
    desk.quarter(1, 4);
    assert_eq!(rect(&desk, 1), (80, 25, 80, 24));
    desk.maximize(1);
    assert_eq!(rect(&desk, 1), (0, 1, 160, 48));
    desk.maximize(1);
    assert_eq!(rect(&desk, 1), (80, 25, 80, 24), "Alt+Enter again: back where it was");
    // Moving and resizing stay on the screen and keep a minimum.
    desk.place(2, Rect::new(40, 10, 30, 10));
    desk.move_by(2, -100, -100);
    assert_eq!(rect(&desk, 2), (0, 1, 30, 10));
    desk.move_by(2, 500, 500);
    assert_eq!(rect(&desk, 2), (130, 39, 30, 10));
    desk.resize_by(2, -100, -100);
    assert_eq!(rect(&desk, 2), (130, 39, 16, 4), "not below the minimum");
    desk.resize_by(2, 100, 100);
    assert_eq!(rect(&desk, 2), (130, 39, 30, 10), "not past the screen");
    // Snapping after a move: an edge, a corner, the top; the middle stays.
    let snapped = |desk: &mut Desk, at: Rect| { desk.place(2, at); desk.snap(2); rect(desk, 2) };
    assert_eq!(snapped(&mut desk, Rect::new(1, 10, 30, 10)), (0, 1, 80, 48), "left edge: left half");
    assert_eq!(snapped(&mut desk, Rect::new(129, 10, 30, 10)), (80, 1, 80, 48), "right edge: right half");
    assert_eq!(snapped(&mut desk, Rect::new(2, 2, 30, 10)), (0, 1, 80, 24), "top left corner: a quarter");
    assert_eq!(snapped(&mut desk, Rect::new(130, 38, 30, 10)), (80, 25, 80, 24), "bottom right corner: a quarter");
    assert_eq!(snapped(&mut desk, Rect::new(50, 25, 30, 24)), (0, 25, 160, 24), "bottom edge: bottom half");
    assert_eq!(snapped(&mut desk, Rect::new(50, 2, 30, 10)), (0, 1, 160, 48), "top edge: the whole screen");
    desk.maximize(2);
    assert_eq!(rect(&desk, 2), (50, 2, 30, 10), "restored to where it was before snapping to the top");
    assert_eq!(snapped(&mut desk, Rect::new(50, 20, 30, 10)), (50, 20, 30, 10), "the middle: no snapping");
    assert!(desk.take_changed().contains(&2));
    assert!(desk.take_changed().is_empty());
}

#[test]
fn what_the_mouse_hits() {
    let desk = desk_of_four();
    // Window 2 is at (80, 1) 80 × 24: the title row, the [×] mark near its right end, the ◆ corner, the border.
    assert_eq!(desk.hit(100, 1), Hit::Title(2));
    assert_eq!(desk.hit(156, 1), Hit::Close(2));
    assert_eq!(desk.hit(155, 1), Hit::Close(2));
    assert_eq!(desk.hit(158, 1), Hit::Title(2));
    assert_eq!(desk.hit(159, 24), Hit::Corner(2));
    assert_eq!(desk.hit(80, 10), Hit::Border(2));
    assert_eq!(desk.hit(100, 10), Hit::Content(2));
    assert_eq!(desk.hit(100, 0), Hit::Desktop, "the top bar");
    assert_eq!(desk.hit(150, 40), Hit::Desktop);
    // Overlap: the window in front decides.
    let mut desk = desk;
    desk.place(3, Rect::new(70, 5, 42, 13));
    desk.raise(3);
    assert_eq!(desk.hit(100, 10), Hit::Content(3));
    desk.raise(2);
    assert_eq!(desk.hit(100, 10), Hit::Content(2));
}

#[test]
fn keys_wm_keeps_and_passes_on() {
    let mut wm = Wm::new(160, 50);
    // An empty desktop: Enter asks for a program.
    assert_eq!(wm.key(key(KEY_ENTER)), Action::Redraw);
    for ch in "fm data".chars() { wm.key(chr(ch)); }
    assert_eq!(wm.key(key(KEY_ENTER)), Action::Run("fm data".into()));
    wm.desk = desk_of_four();
    assert_eq!(wm.key(chr('x')), Action::Forward, "plain keys go to the window in front");
    assert_eq!(wm.key(key(KEY_F1)), Action::Forward);
    assert_eq!(wm.key(Key(event(KEY_F1, 0, MOD_ALT))), Action::Forward, "Alt+F1 is fm's (the left panel's volume)");
    assert_eq!(wm.key(Key(event(KEY_F1 + 1, 0, MOD_ALT))), Action::Forward);
    assert_eq!(wm.key(Key(event(KEY_F1 + 6, 0, MOD_ALT))), Action::Forward, "fm's Alt+F7");
    assert_eq!(wm.key(Key(event(KEY_F1 + 7, 0, MOD_ALT))), Action::Forward, "edit's Alt+F8");
    assert_eq!(wm.key(Key(event(KEY_BACKSPACE, 8, MOD_ALT))), Action::Forward, "edit's Alt+Backspace");
    assert_eq!(wm.key(alt_char('h')), Action::Redraw, "Alt+H: wm's keys");
    assert!(wm.status().starts_with("MODE=HELP"));
    wm.key(key(KEY_ESC));
    assert_eq!(wm.key(alt(KEY_TAB, '\t')), Action::Redraw);
    assert_eq!(wm.desk.focus(), Some(1));
    assert_eq!(wm.key(Key(event(KEY_TAB, '\t' as u32, MOD_ALT | MOD_SHIFT))), Action::Redraw);
    assert_eq!(wm.desk.focus(), Some(4));
    wm.key(alt(KEY_LEFT, '\0'));
    assert_eq!(rect(&wm.desk, 4), (0, 1, 80, 48));
    wm.key(alt_char('2'));
    assert_eq!(rect(&wm.desk, 4), (80, 1, 80, 24));
    wm.key(alt(KEY_ENTER, '\n'));
    assert_eq!(rect(&wm.desk, 4), (0, 1, 160, 48));
    wm.key(alt(KEY_ENTER, '\n'));
    assert_eq!(rect(&wm.desk, 4), (80, 1, 80, 24));
    // Alt+M: arrows move, Shift+arrows resize, Esc goes back, Enter snaps.
    wm.key(alt_char('m'));
    assert!(wm.status().starts_with("MODE=MOVE"));
    assert_eq!(wm.key(chr('q')), Action::Redraw, "nothing reaches the program while moving");
    wm.key(key(KEY_DOWN));
    wm.key(Key(event(KEY_LEFT, 0, MOD_SHIFT)));
    assert_eq!(rect(&wm.desk, 4), (80, 2, 79, 24));
    wm.key(key(KEY_ESC));
    assert_eq!(rect(&wm.desk, 4), (80, 1, 80, 24));
    wm.key(alt_char('m'));
    for _ in 0..10 { wm.key(key(KEY_DOWN)); }
    for _ in 0..3 { wm.key(Key(event(KEY_RIGHT, 0, MOD_CTRL))); }
    assert_eq!(rect(&wm.desk, 4), (80, 11, 80, 24), "stays on the screen");
    for _ in 0..90 { wm.key(key(KEY_LEFT)); }
    wm.key(key(KEY_ENTER));
    assert_eq!(rect(&wm.desk, 4), (0, 1, 80, 48), "at the left edge it snaps to the left half");
    assert!(wm.status().starts_with("MODE=NORMAL FOCUS=4"));
    // Close, run, leave, close all; the Russian layout gives the same letters.
    assert_eq!(wm.key(alt_char('w')), Action::Close(4));
    assert_eq!(wm.key(Key(event(KEY_F1 + 3, 0, MOD_ALT))), Action::Close(4));
    assert_eq!(wm.key(alt_char('ц')), Action::Close(4));
    wm.key(alt_char('к'));
    assert!(wm.status().starts_with("MODE=RUN"));
    for ch in "clock".chars() { wm.key(chr(ch)); }
    assert_eq!(wm.key(key(KEY_ENTER)), Action::Run("clock".into()));
    assert_eq!(wm.key(alt_char('q')), Action::Detach);
    assert_eq!(wm.key(alt_char('x')), Action::CloseAll);
    assert_eq!(wm.key(alt_char('z')), Action::Forward, "other Alt keys are the program's");
}

#[test]
fn the_mouse_drags_titles_and_corners() {
    let mut wm = Wm::new(160, 50);
    wm.desk = desk_of_four();
    // A click in window 1 brings it to the front and goes to its program, at the cell of its content.
    assert_eq!(rect(&wm.desk, 1), (0, 1, 80, 24));
    assert_eq!(wm.pointer(10, 10, 1, 0), Action::Pointer { id: 1, x: 9, y: 8, buttons: 1, wheel: 0 });
    assert_eq!(wm.pointer(10, 10, 0, 0), Action::Pointer { id: 1, x: 9, y: 8, buttons: 0, wheel: 0 });
    assert_eq!(wm.desk.focus(), Some(1));
    // Drag window 2's title to the left edge: it follows, then snaps to the left half.
    wm.pointer(100, 1, 0, 0);
    wm.pointer(100, 1, 1, 0);
    assert_eq!(wm.desk.focus(), Some(2));
    wm.pointer(60, 20, 1, 0);
    assert_eq!(rect(&wm.desk, 2), (40, 20, 80, 24));
    wm.pointer(20, 20, 1, 0);
    assert_eq!(rect(&wm.desk, 2), (0, 20, 80, 24));
    wm.pointer(20, 20, 0, 0);
    assert_eq!(rect(&wm.desk, 2), (0, 1, 80, 48));
    // The corner resizes.
    wm.desk.place(2, Rect::new(40, 10, 30, 10));
    wm.pointer(69, 19, 1, 0);
    wm.pointer(89, 24, 1, 0);
    wm.pointer(89, 24, 0, 0);
    assert_eq!(rect(&wm.desk, 2), (40, 10, 50, 15));
    // [×] closes.
    assert_eq!(wm.pointer(86, 10, 1, 0), Action::Close(2));
    wm.pointer(86, 10, 0, 0);
    assert_eq!(wm.pointer.unwrap(), (86, 10));
}

#[test]
fn snapped_windows_give_their_size_back() {
    let mut wm = Wm::new(160, 50);
    wm.desk = desk_of_four();
    let title = |wm: &mut Wm, id: u32| {
        let mut cells = vec![Cell::BLANK; 160 * 50];
        let mut grid = Grid::new(&mut cells, 160, 50);
        let mut text = |_: u32, _: usize, _: usize| None;
        wm.draw(&mut grid, &DARK, &mut text, None);
        let r = wm.desk.get(id).unwrap().rect;
        (r.x..r.right()).map(|x| grid.get(x, r.y).ch).collect::<String>()
    };
    // Window 2 floats at (80, 1) 80 × 24: [▲] maximizes it, [⇕] gives its frame back.
    assert!(title(&mut wm, 2).contains("[▲][×]"), "{}", title(&mut wm, 2));
    assert_eq!(wm.desk.hit(153, 1), Hit::Zoom(2));
    wm.pointer(153, 1, 1, 0);
    wm.pointer(153, 1, 0, 0);
    assert_eq!(rect(&wm.desk, 2), (0, 1, 160, 48));
    assert!(title(&mut wm, 2).contains("[⇕][×]"));
    wm.pointer(153, 1, 1, 0);
    wm.pointer(153, 1, 0, 0);
    assert_eq!((rect(&wm.desk, 2), wm.desk.get(2).unwrap().restore), ((80, 1, 80, 24), None));
    // A window snapped to the left half by a key: [⇕] gives back the frame it floated in.
    wm.key(alt(KEY_LEFT, '\0'));
    assert_eq!(rect(&wm.desk, 2), (0, 1, 80, 48));
    assert!(title(&mut wm, 2).contains("[⇕][×]"));
    // A click on its title does not move it ...
    wm.pointer(30, 1, 1, 0);
    wm.pointer(30, 1, 0, 0);
    assert_eq!(rect(&wm.desk, 2), (0, 1, 80, 48));
    // ... a drag does: it leaves the edge with that frame, held at the same share of its width, and floats.
    wm.pointer(40, 1, 1, 0);
    wm.pointer(60, 10, 1, 0);
    assert_eq!(rect(&wm.desk, 2), (20, 10, 80, 24));
    wm.pointer(60, 10, 0, 0);
    assert_eq!((rect(&wm.desk, 2), wm.desk.get(2).unwrap().restore), ((20, 10, 80, 24), None));
    // Maximized by Alt+Enter, then dragged by the title: the same.
    wm.key(alt(KEY_ENTER, '\n'));
    assert_eq!(rect(&wm.desk, 2), (0, 1, 160, 48));
    wm.pointer(80, 1, 1, 0);
    wm.pointer(80, 6, 1, 0);
    wm.pointer(80, 6, 0, 0);
    assert_eq!(rect(&wm.desk, 2), (40, 6, 80, 24));
    // Snapped again at the right edge, then given back with [⇕]; Alt+M and Esc keep what [⇕] gives back.
    wm.pointer(80, 6, 1, 0);
    wm.pointer(158, 6, 1, 0);
    wm.pointer(158, 6, 0, 0);
    assert_eq!(rect(&wm.desk, 2), (80, 1, 80, 48));
    wm.key(alt_char('m'));
    wm.key(key(KEY_LEFT));
    wm.key(key(KEY_ESC));
    assert_eq!(rect(&wm.desk, 2), (80, 1, 80, 48));
    wm.pointer(153, 1, 1, 0);
    wm.pointer(153, 1, 0, 0);
    assert_eq!(rect(&wm.desk, 2), (80, 6, 80, 24), "where it was let go (kept on the screen)");
}

#[test]
fn the_mouse_goes_to_the_programs() {
    let mut wm = Wm::new(160, 50);
    wm.desk = desk_of_four();
    let inner = wm.desk.get(2).unwrap().rect.inner();
    // Moves with no button held stay with wm.
    assert_eq!(wm.pointer(inner.x + 3, inner.y + 2, 0, 0), Action::Redraw);
    // The wheel goes to the window under the mouse, which stays where it is in the stack.
    assert_eq!(wm.pointer(inner.x + 3, inner.y + 2, 0, -1), Action::Pointer { id: 2, x: 3, y: 2, buttons: 0, wheel: -1 });
    assert_eq!(wm.desk.focus(), Some(4));
    assert_eq!(wm.pointer(0, 0, 0, 1), Action::Redraw, "not over a window's content");
    // A press grabs the mouse: the window gets the moves and the release, at the nearest cell when outside it.
    assert_eq!(wm.pointer(inner.x, inner.y, 2, 0), Action::Pointer { id: 2, x: 0, y: 0, buttons: 2, wheel: 0 }, "the right button too");
    assert_eq!(wm.desk.focus(), Some(2));
    assert_eq!(wm.pointer(5, 40, 2, 0), Action::Pointer { id: 2, x: 0, y: inner.h - 1, buttons: 2, wheel: 0 });
    assert_eq!(wm.pointer(5, 40, 0, 0), Action::Pointer { id: 2, x: 0, y: inner.h - 1, buttons: 0, wheel: 0 });
    assert_eq!(wm.pointer(5, 40, 0, 0), Action::Redraw, "released");
    // A right click on a frame raises the window; no drag starts.
    let title = wm.desk.get(3).unwrap().rect;
    assert_eq!(wm.pointer(title.x + 2, title.y, 2, 0), Action::Redraw);
    assert_eq!(wm.desk.focus(), Some(3));
    let before = rect(&wm.desk, 3);
    wm.pointer(title.x + 10, title.y + 5, 2, 0);
    wm.pointer(title.x + 10, title.y + 5, 0, 0);
    assert_eq!(rect(&wm.desk, 3), before);
    // In a dialog of wm the mouse goes nowhere.
    wm.key(alt_char('h'));
    assert_eq!(wm.pointer(inner.x + 3, inner.y + 2, 1, 0), Action::Redraw);
    wm.pointer(inner.x + 3, inner.y + 2, 0, 0);
    assert_eq!(wm.pointer(inner.x + 3, inner.y + 2, 0, 1), Action::Redraw);
}

#[test]
fn what_each_cell_shows() {
    let mut desk = desk_of_four();
    desk.place(3, Rect::new(70, 5, 42, 13)); // the clock under window 2's corner
    desk.raise(2);
    let mut cells = vec![Cell::BLANK; 160 * 50];
    let mut grid = Grid::new(&mut cells, 160, 50);
    let mut text = |id: u32, x: usize, y: usize| Some((if id == 2 && y == 0 { "A:/data".chars().nth(x).unwrap_or(' ') } else { '.' }, 0xFFFFFF, 0x0000AA));
    let owner = desk.draw(&mut grid, &DARK, &mut text);
    let row = |y: usize| -> String { (0..160).map(|x| grid.get(x, y).ch).collect() };
    assert!(row(1).starts_with("┌") && row(1).contains(" fm ") && row(1).contains("╔"), "{}", row(1));
    assert!(row(1).contains("[×]═╗"), "the focused window: a double frame and its close mark: {}", row(1));
    assert!(row(2)[row(2).char_indices().nth(81).unwrap().0..].starts_with("A:/data"), "text content: {}", row(2));
    assert_eq!(grid.get(1, 2).style.bg, 0x0000AA, "the program's colours");
    assert_eq!(grid.get(159, 24).ch, '◆');
    // Pixel windows: their content cells, minus what is in front of them.
    let clock = desk.index(3).unwrap() as u16 + 1;
    assert_eq!(owner[6 * 160 + 71], clock);
    assert_eq!(owner[6 * 160 + 81], 0, "covered by window 2");
    assert_eq!(owner[5 * 160 + 71], 0, "its own frame");
    let dzen = desk.index(4).unwrap() as u16 + 1;
    assert_eq!(owner[30 * 160 + 90], dzen);
    assert_eq!(grid.get(150, 40).ch, '░', "the desktop");
    // The whole manager: the top bar, the status line, the run dialog's cursor.
    let mut wm = Wm::new(160, 50);
    wm.desk = desk;
    let mut cells = vec![Cell::BLANK; 160 * 50];
    let mut grid = Grid::new(&mut cells, 160, 50);
    let (_, cursor) = wm.draw(&mut grid, &DARK, &mut text, Some((3, 0)));
    let row = |grid: &Grid, y: usize| -> String { (0..160).map(|x| grid.get(x, y).ch).collect() };
    assert!(row(&grid, 0).starts_with(" wm │ Alt+Tab next") && row(&grid, 0).trim_end().ends_with("fm"), "{}", row(&grid, 0));
    assert!(row(&grid, 49).starts_with("4 windows; keys go to \"fm\""), "{}", row(&grid, 49));
    assert_eq!(cursor, Some((84, 2)), "the focused text window's cursor, inside its frame");
    wm.key(alt_char('r'));
    let (_, cursor) = wm.draw(&mut grid, &DARK, &mut text, Some((3, 0)));
    assert!(cursor.is_some() && cursor != Some((84, 2)));
    // A dialog over a pixel window (Alt+H over the clocks): the cells it covers are not the window's, so its pixels
    // are not copied over the dialog; the rest of the window still shows them.
    wm.key(key(KEY_ESC));
    let mut clocks = Wm::new(160, 50);
    let mut big = pixels(1, "dzen-clock", 1200, 700);
    big.rect = Rect::new(10, 5, 140, 40);
    clocks.desk.add(big);
    let mut cells = vec![Cell::BLANK; 160 * 50];
    let mut grid = Grid::new(&mut cells, 160, 50);
    let (owner, _) = clocks.draw(&mut grid, &DARK, &mut text, None);
    assert_eq!((owner[25 * 160 + 80], owner[6 * 160 + 11]), (1, 1));
    clocks.key(alt_char('h'));
    let (owner, _) = clocks.draw(&mut grid, &DARK, &mut text, None);
    assert_eq!(owner[25 * 160 + 80], 0, "under the help dialog");
    assert_eq!(owner[6 * 160 + 11], 1, "outside the dialog");
    clocks.key(key(KEY_ESC));
    let (owner, _) = clocks.draw(&mut grid, &DARK, &mut text, None);
    assert_eq!(owner[25 * 160 + 80], 1, "the dialog closed: the pixels come back");
    // Every size draws.
    for (cols, rows) in [(20, 6), (40, 12), (100, 30)] {
        let mut wm = Wm::new(cols, rows);
        wm.desk = desk_of_four_on(cols, rows);
        let mut cells = vec![Cell::BLANK; cols * rows];
        let mut grid = Grid::new(&mut cells, cols, rows);
        let _ = wm.draw(&mut grid, &DARK, &mut text, None);
    }
}

fn desk_of_four_on(cols: usize, rows: usize) -> Desk {
    let mut desk = Desk::new(cols, rows);
    desk.add(text(1, "fm"));
    desk.add(pixels(2, "clock", 320, 176));
    desk.add(text(3, "top"));
    desk.add(pixels(4, "dzen-clock", 400, 320));
    desk.add(text(5, "edit"));
    desk
}
