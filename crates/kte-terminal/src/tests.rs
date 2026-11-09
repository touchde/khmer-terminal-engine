use super::*;

fn term(rows: usize, cols: usize) -> Terminal {
    Terminal::new(rows, cols, CellModel::Wide)
}

fn feed(t: &mut Terminal, s: &str) {
    t.feed(s.as_bytes());
}

/// `(text, width)` of each cluster start on a row.
fn cells(t: &Terminal, row: usize) -> Vec<(String, u8)> {
    t.row(row)
        .cells
        .iter()
        .filter(|c| !c.is_continuation() && !c.is_blank())
        .map(|c| (c.text.clone(), c.width))
        .collect()
}

#[test]
fn khmer_clusters_occupy_spec_widths() {
    let mut t = term(2, 20);
    feed(&mut t, "សួស្តី");
    assert_eq!(cells(&t, 0), [("សួ".into(), 3), ("ស្តី".into(), 3)]);
    assert_eq!(t.cursor(), (0, 6));
    assert!(t.cell(0, 1).is_continuation() && t.cell(0, 2).is_continuation());
}

#[test]
fn syllable_arriving_byte_by_byte_is_extended_in_place() {
    let mut t = term(1, 20);
    for b in "ស្ត្រី".as_bytes() {
        t.feed(&[*b]);
    }
    assert_eq!(cells(&t, 0), [("ស្ត្រី".into(), 4)]);
    assert_eq!(t.cursor(), (0, 4));
}

#[test]
fn spacing_vowel_widens_the_previous_cluster() {
    let mut t = term(1, 20);
    feed(&mut t, "ក");
    assert_eq!(t.cursor(), (0, 2));
    feed(&mut t, "ោ");
    assert_eq!(cells(&t, 0), [("កោ".into(), 4)]);
    assert_eq!(t.cursor(), (0, 4));
}

#[test]
fn colors_between_base_and_vowel_still_combine() {
    let mut t = term(1, 20);
    feed(&mut t, "ក\x1b[31mា");
    assert_eq!(cells(&t, 0), [("កា".into(), 3)]);
}

#[test]
fn cursor_movement_breaks_combining() {
    let mut t = term(1, 20);
    feed(&mut t, "ក\x1b[5Gា");
    assert_eq!(cells(&t, 0), [("ក".into(), 2), ("ា".into(), 3)]);
}

#[test]
fn compact_model() {
    let mut t = Terminal::new(1, 20, CellModel::Compact);
    feed(&mut t, "សួស្តី ok");
    assert_eq!(t.cursor(), (0, 7));
    assert_eq!(t.row_text(0), "សួស្តី ok");
}

#[test]
fn clusters_wrap_whole() {
    let mut t = term(3, 5);
    feed(&mut t, "abcសា"); // សា is 4 cells: does not fit after "abc"
    assert_eq!(t.row_text(0), "abc");
    assert_eq!(t.row_text(1), "សា");
    assert!(t.row(0).wrapped);
    assert_eq!(t.selection_text((0, 0), (1, 4)), "abcសា");
}

#[test]
fn growing_cluster_at_line_end_moves_to_next_line() {
    let mut t = term(2, 4);
    feed(&mut t, "abក"); // ក fills cols 2..4
    assert_eq!(t.row_text(0), "abក");
    feed(&mut t, "ា"); // កា needs 3 cells: no longer fits
    assert_eq!(t.row_text(0), "ab");
    assert_eq!(t.row_text(1), "កា");
    assert_eq!(t.cursor(), (1, 3));
}

#[test]
fn exact_fit_defers_wrap() {
    let mut t = term(2, 4);
    feed(&mut t, "ក្ស"); // 3 cells
    feed(&mut t, "a"); // fills col 3
    assert_eq!(t.cursor(), (0, 3));
    feed(&mut t, "b");
    assert_eq!(t.row_text(1), "b");
}

#[test]
fn overwriting_inside_a_cluster_erases_all_of_it() {
    let mut t = term(1, 10);
    feed(&mut t, "កោx");
    feed(&mut t, "\x1b[2GZ"); // column 2 is inside កោ (cols 1..4)
    assert_eq!(t.row_text(0), " Z  x");
    assert!(!t.cell(0, 2).is_continuation());
}

#[test]
fn selection_snaps_to_clusters_and_preserves_codepoints() {
    let mut t = term(2, 30);
    let line = "ខ្ញុំស្រឡាញ់\u{200B}ភាសាខ្មែរ";
    feed(&mut t, line);
    // Select from the middle of the first cluster to the middle of the last.
    let end = t.cursor().1 - 1;
    assert_eq!(t.selection_text((0, 1), (0, end)), line);
}

#[test]
fn zero_width_space_is_kept_but_takes_no_cell() {
    let mut t = term(1, 10);
    feed(&mut t, "ក\u{200B}ខ");
    assert_eq!(t.cursor(), (0, 4));
    assert_eq!(t.row_text(0), "ក\u{200B}ខ");
}

#[test]
fn erase_and_cursor_controls() {
    let mut t = term(3, 10);
    feed(&mut t, "hello\r\nworld\x1b[1;3H\x1b[K");
    assert_eq!(t.row_text(0), "he");
    assert_eq!(t.row_text(1), "world");
    feed(&mut t, "\x1b[2J");
    assert_eq!(t.screen_text(), "");
    feed(&mut t, "\x1b[3;4Hx\x08\x08y");
    assert_eq!(t.row_text(2), "  yx"); // x at col 3, two backspaces, y at col 2
}

#[test]
fn backspace_moves_one_cell() {
    let mut t = term(1, 10);
    feed(&mut t, "ក\x08");
    assert_eq!(t.cursor(), (0, 1));
}

#[test]
fn sgr_attributes() {
    let mut t = term(1, 10);
    feed(&mut t, "\x1b[1;31;48;2;1;2;3mA\x1b[0mB\x1b[38;5;200mC");
    let a = t.cell(0, 0).attrs;
    assert!(a.bold);
    assert_eq!(a.fg, Color::Indexed(1));
    assert_eq!(a.bg, Color::Rgb(1, 2, 3));
    assert_eq!(t.cell(0, 1).attrs, Attrs::default());
    assert_eq!(t.cell(0, 2).attrs.fg, Color::Indexed(200));
}

#[test]
fn scrolling_fills_scrollback() {
    let mut t = term(2, 10);
    feed(&mut t, "one\r\ntwo\r\nthree");
    assert_eq!(t.screen_text(), "two\nthree");
    assert_eq!(t.scrollback().len(), 1);
    assert_eq!(t.scrollback()[0].text(), "one");
}

#[test]
fn scroll_region() {
    let mut t = term(4, 10);
    feed(&mut t, "top\r\na\r\nb\r\nbottom\x1b[2;3r\x1b[3;1H\nc");
    assert_eq!(t.screen_text(), "top\nb\nc\nbottom");
}

#[test]
fn insert_and_delete_cells_repair_clusters() {
    let mut t = term(1, 10);
    feed(&mut t, "កាxy\x1b[2G\x1b[P"); // delete inside កា
    assert_eq!(t.row_text(0), "  xy".trim_end());
    assert!(t.row(0).cells.iter().all(|c| c.width <= 1));
    let mut t = term(1, 6);
    feed(&mut t, "abកា\x1b[1G\x1b[2@"); // push កា past the edge
    assert_eq!(t.row_text(0), "  ab");
}

#[test]
fn alternate_screen_restores_main() {
    let mut t = term(2, 10);
    feed(&mut t, "main\x1b[?1049h");
    assert!(t.alt_screen());
    feed(&mut t, "full screen app");
    feed(&mut t, "\x1b[?1049l");
    assert!(!t.alt_screen());
    assert_eq!(t.row_text(0), "main");
    assert_eq!(t.cursor(), (0, 4));
}

#[test]
fn queries_get_responses() {
    let mut t = term(5, 10);
    feed(&mut t, "\x1b[3;4H\x1b[6n\x1b[?2027$p\x1b[?7$p");
    assert_eq!(t.take_responses(), b"\x1b[3;4R\x1b[?2027;3$y\x1b[?7;1$y");
    assert!(t.take_responses().is_empty());
}

#[test]
fn title_and_tabs() {
    let mut t = term(1, 20);
    feed(&mut t, "\x1b]0;សួស្តី\x07a\tb");
    assert_eq!(t.title(), "សួស្តី");
    assert_eq!(t.cursor(), (0, 9));
}

#[test]
fn resize_keeps_content_and_erases_cut_clusters() {
    let mut t = term(2, 10);
    feed(&mut t, "abcកា");
    t.resize(3, 4); // កា spans cols 3..6: cut by the new edge
    assert_eq!(t.row_text(0), "abc");
    assert_eq!((t.rows(), t.cols()), (3, 4));
}

#[test]
fn reset_clears_everything() {
    let mut t = term(2, 10);
    feed(&mut t, "\x1b[1mtext\x1bc");
    assert_eq!(t.screen_text(), "");
    assert_eq!(t.cursor(), (0, 0));
}

#[test]
fn every_corpus_line_round_trips_through_the_grid() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/khmer_corpus.json"
    );
    let corpus = std::fs::read_to_string(path).unwrap();
    // Minimal extraction of "text" fields, to avoid a JSON dependency here.
    for line in corpus
        .lines()
        .filter(|l| l.trim_start().starts_with("\"text\""))
    {
        let raw = line
            .trim()
            .trim_start_matches("\"text\": \"")
            .trim_end_matches("\",");
        let text = raw.replace("\\\"", "\"").replace("\\u200b", "\u{200B}");
        let width = kte_unicode::str_width(&text, CellModel::Wide);
        let mut t = term(1, width.max(1));
        feed(&mut t, &text);
        assert_eq!(
            t.selection_text((0, 0), (0, width.max(1) - 1)),
            text,
            "{text}"
        );
    }
}
