use std::io::{self, Write};
use std::ops::Range;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::{cursor, execute, queue, terminal};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32String};

const PAGE: usize = 10;
const PROMPT: &str = "> ";
const HINT: &str = "type to filter, tab or arrows to move, enter to go, esc to cancel";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Insert(char),
    Backspace,
    DeleteWord,
    Clear,
    Up,
    Down,
    Next,
    Previous,
    First,
    Last,
    Accept,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Picked(usize),
    Cancelled,
}

pub fn action(event: KeyEvent) -> Option<Action> {
    let control = event.modifiers.contains(KeyModifiers::CONTROL);
    Some(match (event.code, control) {
        (KeyCode::Char('c' | 'g'), true) => Action::Cancel,
        (KeyCode::Char('p'), true) => Action::Up,
        (KeyCode::Char('n'), true) => Action::Down,
        (KeyCode::Char('u'), true) => Action::Clear,
        (KeyCode::Char('w'), true) => Action::DeleteWord,
        (KeyCode::Char(letter), false) => Action::Insert(letter),
        (KeyCode::Backspace, _) => Action::Backspace,
        (KeyCode::Up, _) => Action::Up,
        (KeyCode::Down, _) => Action::Down,
        (KeyCode::Tab, _) => Action::Next,
        (KeyCode::BackTab, _) => Action::Previous,
        (KeyCode::Home, _) => Action::First,
        (KeyCode::End, _) => Action::Last,
        (KeyCode::Enter, _) => Action::Accept,
        (KeyCode::Esc, _) => Action::Cancel,
        _ => return None,
    })
}

pub struct Entry {
    pub label: String,
    pub search_end: usize,
}

struct Item {
    label: String,
    haystack: Utf32String,
}

struct Hit {
    index: usize,
    matched: Vec<u32>,
}

pub struct Picker {
    items: Vec<Item>,
    matcher: Matcher,
    query: String,
    hits: Vec<Hit>,
    cursor: usize,
    offset: usize,
}

impl Picker {
    pub fn new(entries: Vec<Entry>, start: usize) -> Self {
        let items = entries
            .into_iter()
            .map(|entry| Item {
                haystack: entry
                    .label
                    .chars()
                    .take(entry.search_end)
                    .collect::<String>()
                    .as_str()
                    .into(),
                label: entry.label,
            })
            .collect();
        let mut picker = Self {
            items,
            matcher: Matcher::new(Config::DEFAULT),
            query: String::new(),
            hits: Vec::new(),
            cursor: 0,
            offset: 0,
        };
        picker.search();
        picker.cursor = start.min(picker.hits.len().saturating_sub(1));
        picker
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn label(&self, index: usize) -> &str {
        &self.items[index].label
    }

    #[cfg(test)]
    fn selected(&self) -> usize {
        self.hits.get(self.cursor).map_or(0, |hit| hit.index)
    }

    pub fn visible(&self) -> impl Iterator<Item = (usize, &[u32])> {
        self.hits
            .iter()
            .map(|hit| (hit.index, hit.matched.as_slice()))
    }

    pub fn window(&mut self, height: usize) -> Range<usize> {
        let height = height.max(1);
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + height {
            self.offset = self.cursor + 1 - height;
        }
        self.offset = self.offset.min(self.hits.len().saturating_sub(height));
        self.offset..(self.offset + height).min(self.hits.len())
    }

    pub fn apply(&mut self, action: Action) -> Option<Outcome> {
        let count = self.hits.len();
        match action {
            Action::Insert(letter) => {
                self.query.push(letter);
                self.search();
            }
            Action::Backspace => {
                self.query.pop();
                self.search();
            }
            Action::DeleteWord => {
                let kept = self.query.trim_end().rfind(' ').map_or(0, |at| at + 1);
                self.query.truncate(kept);
                self.search();
            }
            Action::Clear => {
                self.query.clear();
                self.search();
            }
            Action::Up => self.cursor = self.cursor.saturating_sub(1),
            Action::Down => self.cursor = (self.cursor + 1).min(count.saturating_sub(1)),
            Action::Next if count > 0 => self.cursor = (self.cursor + 1) % count,
            Action::Previous if count > 0 => self.cursor = (self.cursor + count - 1) % count,
            Action::First => self.cursor = 0,
            Action::Last => self.cursor = count.saturating_sub(1),
            Action::Accept => {
                return self
                    .hits
                    .get(self.cursor)
                    .map(|hit| Outcome::Picked(hit.index));
            }
            Action::Cancel => return Some(Outcome::Cancelled),
            Action::Next | Action::Previous => {}
        }
        None
    }

    fn search(&mut self) {
        self.cursor = 0;
        self.offset = 0;
        if self.query.is_empty() {
            self.hits = (0..self.items.len())
                .map(|index| Hit {
                    index,
                    matched: Vec::new(),
                })
                .collect();
            return;
        }
        let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
        let mut scored: Vec<(u32, Hit)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let mut matched = Vec::new();
                let score =
                    pattern.indices(item.haystack.slice(..), &mut self.matcher, &mut matched)?;
                matched.sort_unstable();
                matched.dedup();
                Some((score, Hit { index, matched }))
            })
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        self.hits = scored.into_iter().map(|(_, hit)| hit).collect();
    }
}

pub fn pick(entries: Vec<Entry>, start: usize) -> io::Result<Option<usize>> {
    let mut picker = Picker::new(entries, start);
    let mut screen = Screen::open()?;
    screen.draw(&mut picker)?;
    loop {
        let outcome = match crossterm::event::read()? {
            Event::Key(event) if event.kind != KeyEventKind::Release => {
                action(event).and_then(|action| picker.apply(action))
            }
            Event::Resize(..) => {
                screen.draw(&mut picker)?;
                None
            }
            _ => None,
        };
        match outcome {
            Some(Outcome::Picked(index)) => return Ok(Some(index)),
            Some(Outcome::Cancelled) => return Ok(None),
            None => screen.draw(&mut picker)?,
        }
    }
}

struct Screen {
    out: io::Stderr,
}

impl Screen {
    fn open() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self { out: io::stderr() })
    }

    fn draw(&mut self, picker: &mut Picker) -> io::Result<()> {
        let (columns, rows) = terminal::size()?;
        let height = PAGE.min(usize::from(rows).saturating_sub(3)).max(1);
        let width = usize::from(columns).saturating_sub(2);
        self.clear()?;
        queue!(
            self.out,
            SetForegroundColor(Color::Cyan),
            Print(PROMPT),
            ResetColor,
            Print(picker.query()),
            Print("\r\n")
        )?;
        let window = picker.window(height);
        let visible: Vec<_> = picker.visible().collect();
        for (position, (index, matched)) in visible[window.clone()].iter().enumerate() {
            let selected = window.start + position == picker.cursor;
            self.line(picker.label(*index), matched, selected, width)?;
        }
        if visible.is_empty() {
            queue!(
                self.out,
                SetForegroundColor(Color::DarkGrey),
                Print("  no match"),
                ResetColor,
                Print("\r\n")
            )?;
        }
        queue!(
            self.out,
            SetForegroundColor(Color::DarkGrey),
            Print(HINT),
            ResetColor
        )?;
        let drawn = 1 + window.len().max(1) as u16;
        let caret = (PROMPT.chars().count() + picker.query().chars().count()) as u16;
        queue!(self.out, cursor::MoveUp(drawn), cursor::MoveToColumn(caret))?;
        self.out.flush()
    }

    fn line(
        &mut self,
        label: &str,
        matched: &[u32],
        selected: bool,
        width: usize,
    ) -> io::Result<()> {
        let pointer = if selected { "> " } else { "  " };
        queue!(
            self.out,
            SetForegroundColor(Color::Cyan),
            Print(pointer),
            ResetColor
        )?;
        if selected {
            queue!(self.out, SetAttribute(Attribute::Bold))?;
        }
        for (position, letter) in label.chars().take(width).enumerate() {
            let hit = matched.binary_search(&(position as u32)).is_ok();
            if hit {
                queue!(
                    self.out,
                    SetForegroundColor(Color::Magenta),
                    Print(letter),
                    ResetColor
                )?;
            } else {
                queue!(self.out, Print(letter))?;
            }
            if hit && selected {
                queue!(self.out, SetAttribute(Attribute::Bold))?;
            }
        }
        queue!(self.out, SetAttribute(Attribute::Reset), Print("\r\n"))
    }

    fn clear(&mut self) -> io::Result<()> {
        queue!(
            self.out,
            cursor::MoveToColumn(0),
            terminal::Clear(terminal::ClearType::FromCursorDown)
        )
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(
            self.out,
            cursor::MoveToColumn(0),
            terminal::Clear(terminal::ClearType::FromCursorDown)
        );
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(letter: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL)
    }

    fn entries(labels: &[&str], search_end: usize) -> Vec<Entry> {
        labels
            .iter()
            .map(|label| Entry {
                label: label.to_string(),
                search_end,
            })
            .collect()
    }

    fn picker() -> Picker {
        Picker::new(
            entries(
                &[
                    "* app        main       ~/Developer/app",
                    "  fix-login  fix-login  ~/.worktrees/app/fix-login",
                    "  spike      spike      ~/.worktrees/app/spike",
                    "  docs       docs/site  ~/.worktrees/app/docs",
                ],
                24,
            ),
            0,
        )
    }

    fn visible_names(picker: &Picker) -> Vec<String> {
        picker
            .visible()
            .map(|(index, _)| {
                picker
                    .label(index)
                    .trim_start_matches(['*', ' '])
                    .split(' ')
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn keys_map_to_actions() {
        let cases = [
            (key(KeyCode::Char('a')), Some(Action::Insert('a'))),
            (key(KeyCode::Backspace), Some(Action::Backspace)),
            (key(KeyCode::Up), Some(Action::Up)),
            (key(KeyCode::Down), Some(Action::Down)),
            (key(KeyCode::Tab), Some(Action::Next)),
            (key(KeyCode::BackTab), Some(Action::Previous)),
            (key(KeyCode::Home), Some(Action::First)),
            (key(KeyCode::End), Some(Action::Last)),
            (key(KeyCode::Enter), Some(Action::Accept)),
            (key(KeyCode::Esc), Some(Action::Cancel)),
            (ctrl('c'), Some(Action::Cancel)),
            (ctrl('g'), Some(Action::Cancel)),
            (ctrl('p'), Some(Action::Up)),
            (ctrl('n'), Some(Action::Down)),
            (ctrl('u'), Some(Action::Clear)),
            (ctrl('w'), Some(Action::DeleteWord)),
            (key(KeyCode::F(1)), None),
        ];
        for (event, expected) in cases {
            assert_eq!(action(event), expected, "{event:?}");
        }
    }

    #[test]
    fn an_empty_query_shows_every_row_in_order_with_the_cursor_on_the_start_row() {
        let picker = Picker::new(entries(&["a", "b", "c"], 1), 2);
        assert_eq!(visible_names(&picker), ["a", "b", "c"]);
        assert_eq!(picker.selected(), 2);
    }

    #[test]
    fn typing_filters_fuzzily_and_moves_the_cursor_to_the_best_match() {
        let mut picker = picker();
        assert_eq!(picker.apply(Action::Insert('s')), None);
        assert_eq!(picker.apply(Action::Insert('p')), None);
        assert_eq!(visible_names(&picker), ["spike"]);
        assert_eq!(picker.selected(), 2);
    }

    #[test]
    fn a_query_matches_the_branch_but_not_the_path() {
        let mut picker = picker();
        for letter in "site".chars() {
            picker.apply(Action::Insert(letter));
        }
        assert_eq!(visible_names(&picker), ["docs"]);
        picker.apply(Action::Clear);
        for letter in "worktrees".chars() {
            picker.apply(Action::Insert(letter));
        }
        assert!(visible_names(&picker).is_empty());
    }

    #[test]
    fn backspace_and_clear_widen_the_list_again() {
        let mut picker = picker();
        for letter in "spx".chars() {
            picker.apply(Action::Insert(letter));
        }
        assert!(visible_names(&picker).is_empty());
        picker.apply(Action::Backspace);
        assert_eq!(visible_names(&picker), ["spike"]);
        picker.apply(Action::Clear);
        assert_eq!(visible_names(&picker).len(), 4);
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn delete_word_removes_the_last_word() {
        let mut picker = picker();
        for letter in "fix lo".chars() {
            picker.apply(Action::Insert(letter));
        }
        picker.apply(Action::DeleteWord);
        assert_eq!(picker.query(), "fix ");
        picker.apply(Action::DeleteWord);
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn tab_wraps_forward_and_shift_tab_wraps_back() {
        let mut picker = picker();
        picker.apply(Action::Next);
        assert_eq!(picker.selected(), 1);
        picker.apply(Action::Last);
        assert_eq!(picker.selected(), 3);
        picker.apply(Action::Next);
        assert_eq!(picker.selected(), 0);
        picker.apply(Action::Previous);
        assert_eq!(picker.selected(), 3);
    }

    #[test]
    fn arrows_stop_at_the_ends() {
        let mut picker = picker();
        picker.apply(Action::Up);
        assert_eq!(picker.selected(), 0);
        picker.apply(Action::Last);
        picker.apply(Action::Down);
        assert_eq!(picker.selected(), 3);
        picker.apply(Action::First);
        assert_eq!(picker.selected(), 0);
    }

    #[test]
    fn accept_returns_the_selected_row_and_cancel_returns_nothing() {
        let mut picker = picker();
        picker.apply(Action::Next);
        assert_eq!(picker.apply(Action::Accept), Some(Outcome::Picked(1)));
        assert_eq!(picker.apply(Action::Cancel), Some(Outcome::Cancelled));
    }

    #[test]
    fn accept_on_an_empty_list_does_nothing() {
        let mut picker = picker();
        for letter in "zzz".chars() {
            picker.apply(Action::Insert(letter));
        }
        assert_eq!(picker.apply(Action::Accept), None);
    }

    #[test]
    fn matched_characters_are_reported_by_position() {
        let mut picker = picker();
        for letter in "app".chars() {
            picker.apply(Action::Insert(letter));
        }
        let (index, matched) = picker.visible().next().unwrap();
        assert_eq!(index, 0);
        assert_eq!(matched, [2, 3, 4]);
        picker.apply(Action::Clear);
        for letter in "sit".chars() {
            picker.apply(Action::Insert(letter));
        }
        let (index, matched) = picker.visible().next().unwrap();
        assert_eq!(index, 3);
        assert_eq!(matched, [18, 19, 20]);
    }

    #[test]
    fn the_window_keeps_the_cursor_visible() {
        let labels: Vec<String> = (0..20).map(|n| n.to_string()).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut picker = Picker::new(entries(&labels, 2), 0);
        assert_eq!(picker.window(5), 0..5);
        picker.apply(Action::Last);
        assert_eq!(picker.window(5), 15..20);
        picker.apply(Action::Up);
        assert_eq!(picker.window(5), 15..20);
        picker.apply(Action::First);
        assert_eq!(picker.window(5), 0..5);
    }
}
