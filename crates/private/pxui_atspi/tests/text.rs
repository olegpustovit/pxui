//! The text helpers the AT-SPI bridge answers with (they build everywhere; the bridge is Linux only).
use pxui_atspi::map::{text_edit, text_span};

#[test]
fn text_edit_is_the_changed_middle() {
    assert_eq!(text_edit("hello world", "hello brave world"), (6, "", "brave "));
    assert_eq!(text_edit("café au lait", "café noir"), (5, "au lait", "noir"));
    assert_eq!(text_edit("aaa", "aa"), (2, "a", ""));
    assert_eq!(text_edit("", "x"), (0, "", "x"));
}

#[test]
fn text_span_by_granularity() {
    let t = "One fish. Two fish!\nRed fish";
    let at = |k: i32, g: u32| { let (s, e) = text_span(t, k, g); t.chars().skip(s as usize).take((e - s) as usize).collect::<String>() };
    assert_eq!(at(1, 0), "n");
    assert_eq!(at(5, 1), "fish");
    assert_eq!(at(3, 1), " ");
    assert_eq!(at(12, 2), "Two fish!\n");
    assert_eq!(at(2, 2), "One fish. ");
    assert_eq!(at(22, 3), "Red fish");
    assert_eq!(at(0, 4), "One fish. Two fish!\n");
}
