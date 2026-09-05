use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Column {
    Name,
    Branch,
    Head,
    State,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Field {
    Path,
    Head,
    Branch,
    Bare,
    Locked,
    Prunable,
    Current,
}

pub struct Row<'a> {
    pub worktree: &'a w3::Worktree,
    pub current: bool,
}

impl FromStr for Column {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "name" => Ok(Column::Name),
            "branch" => Ok(Column::Branch),
            "head" => Ok(Column::Head),
            "state" => Ok(Column::State),
            "path" => Ok(Column::Path),
            other => Err(format!("unknown column: {other}")),
        }
    }
}

impl TryFrom<String> for Column {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl Column {
    fn header(self) -> &'static str {
        match self {
            Column::Name => "NAME",
            Column::Branch => "BRANCH",
            Column::Head => "HEAD",
            Column::State => "STATE",
            Column::Path => "PATH",
        }
    }
}

impl FromStr for Field {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "path" => Ok(Field::Path),
            "head" => Ok(Field::Head),
            "branch" => Ok(Field::Branch),
            "bare" => Ok(Field::Bare),
            "locked" => Ok(Field::Locked),
            "prunable" => Ok(Field::Prunable),
            "current" => Ok(Field::Current),
            other => Err(format!("unknown field: {other}")),
        }
    }
}

impl TryFrom<String> for Field {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl Field {
    fn key(self) -> &'static str {
        match self {
            Field::Path => "path",
            Field::Head => "head",
            Field::Branch => "branch",
            Field::Bare => "bare",
            Field::Locked => "locked",
            Field::Prunable => "prunable",
            Field::Current => "current",
        }
    }
}

pub fn parse_list<T: FromStr<Err = String>>(text: &str) -> Result<Vec<T>, String> {
    let items: Vec<T> = text
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    if items.is_empty() {
        return Err("empty list".to_string());
    }
    Ok(items)
}

pub fn table(rows: &[Row], columns: &[Column], head_length: usize, home: Option<&Path>) -> String {
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|column| table_cell(row, *column, head_length, home))
                .collect()
        })
        .collect();
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            cells
                .iter()
                .map(|row| row[index].chars().count())
                .chain(std::iter::once(column.header().chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let header: Vec<String> = columns
        .iter()
        .map(|column| column.header().to_string())
        .collect();
    let mut out = table_line(' ', &header, &widths);
    for (row, row_cells) in rows.iter().zip(&cells) {
        let marker = if row.current { '*' } else { ' ' };
        out.push_str(&table_line(marker, row_cells, &widths));
    }
    out
}

fn table_line(marker: char, cells: &[String], widths: &[usize]) -> String {
    let mut line = marker.to_string();
    for (cell, width) in cells.iter().zip(widths) {
        line.push(' ');
        line.push_str(cell);
        let padding = width.saturating_sub(cell.chars().count());
        line.extend(std::iter::repeat_n(' ', padding + 1));
    }
    let trimmed = line.trim_end();
    format!("{trimmed}\n")
}

fn table_cell(row: &Row, column: Column, head_length: usize, home: Option<&Path>) -> String {
    match column {
        Column::Name => name(&row.worktree.path),
        Column::Path => home_shortened(&row.worktree.path, home),
        Column::State => state(row, false),
        Column::Branch | Column::Head => plain_cell(row, column, head_length),
    }
}

pub fn plain(rows: &[Row], columns: &[Column], head_length: usize) -> String {
    rows.iter()
        .map(|row| {
            let cells: Vec<String> = columns
                .iter()
                .map(|column| plain_cell(row, *column, head_length))
                .collect();
            format!("{}\n", cells.join("\t"))
        })
        .collect()
}

fn plain_cell(row: &Row, column: Column, head_length: usize) -> String {
    match column {
        Column::Name => name(&row.worktree.path),
        Column::Branch => row.worktree.branch.clone().unwrap_or_default(),
        Column::Head => row.worktree.head.chars().take(head_length).collect(),
        Column::State => state(row, true),
        Column::Path => row.worktree.path.to_string_lossy().into_owned(),
    }
}

pub(crate) fn name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn home_shortened(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.to_string_lossy().into_owned(),
    }
}

fn state(row: &Row, with_current: bool) -> String {
    let worktree = row.worktree;
    let flags = [
        (with_current && row.current, "current"),
        (worktree.bare, "bare"),
        (worktree.locked.is_some(), "locked"),
        (worktree.prunable.is_some(), "prunable"),
    ];
    flags
        .iter()
        .filter(|(set, _)| *set)
        .map(|(_, flag)| *flag)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn json(rows: &[Row], fields: &[Field]) -> String {
    let objects: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut object = serde_json::Map::new();
            for field in fields {
                object.insert(field.key().to_string(), json_value(row, *field));
            }
            serde_json::Value::Object(object)
        })
        .collect();
    format!("{}\n", serde_json::Value::Array(objects))
}

fn json_value(row: &Row, field: Field) -> serde_json::Value {
    let worktree = row.worktree;
    match field {
        Field::Path => worktree.path.to_string_lossy().into(),
        Field::Head => worktree.head.as_str().into(),
        Field::Branch => worktree.branch.as_deref().into(),
        Field::Bare => worktree.bare.into(),
        Field::Locked => worktree.locked.as_deref().into(),
        Field::Prunable => worktree.prunable.as_deref().into(),
        Field::Current => row.current.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const HEAD: &str = "14b96db3c138a070d35201b350cba339eedd99f2";
    const ALL_COLUMNS: [Column; 5] = [
        Column::Name,
        Column::Branch,
        Column::Head,
        Column::State,
        Column::Path,
    ];
    const ALL_FIELDS: [Field; 7] = [
        Field::Path,
        Field::Head,
        Field::Branch,
        Field::Bare,
        Field::Locked,
        Field::Prunable,
        Field::Current,
    ];

    fn worktree(path: &str, branch: Option<&str>) -> w3::Worktree {
        w3::Worktree {
            path: PathBuf::from(path),
            head: HEAD.into(),
            branch: branch.map(Into::into),
            locked: None,
            prunable: None,
            bare: false,
        }
    }

    fn flagged(path: &str) -> w3::Worktree {
        let mut worktree = worktree(path, Some("main"));
        worktree.bare = true;
        worktree.locked = Some(String::new());
        worktree.prunable = Some("gitdir file points to non-existent location".into());
        worktree
    }

    #[test]
    fn column_names_round_trip() {
        for (name, column) in [
            ("name", Column::Name),
            ("branch", Column::Branch),
            ("head", Column::Head),
            ("state", Column::State),
            ("path", Column::Path),
        ] {
            assert_eq!(name.parse::<Column>(), Ok(column));
        }
    }

    #[test]
    fn field_names_round_trip() {
        for (name, field) in [
            ("path", Field::Path),
            ("head", Field::Head),
            ("branch", Field::Branch),
            ("bare", Field::Bare),
            ("locked", Field::Locked),
            ("prunable", Field::Prunable),
            ("current", Field::Current),
        ] {
            assert_eq!(name.parse::<Field>(), Ok(field));
        }
    }

    #[test]
    fn unknown_names_are_reported() {
        assert_eq!(
            "nope".parse::<Column>(),
            Err("unknown column: nope".to_string())
        );
        assert_eq!(
            "nope".parse::<Field>(),
            Err("unknown field: nope".to_string())
        );
    }

    #[test]
    fn parses_a_comma_list_and_trims() {
        assert_eq!(
            parse_list::<Column>("name, path"),
            Ok(vec![Column::Name, Column::Path])
        );
    }

    #[test]
    fn a_bad_list_item_names_itself() {
        assert_eq!(
            parse_list::<Column>("name,nope"),
            Err("unknown column: nope".to_string())
        );
    }

    #[test]
    fn an_empty_list_is_an_error() {
        assert_eq!(parse_list::<Column>(""), Err("empty list".to_string()));
        assert_eq!(parse_list::<Column>(" , "), Err("empty list".to_string()));
    }

    #[test]
    fn plain_is_tab_separated_with_short_head() {
        let main = worktree("/repo", Some("main"));
        let rows = [Row {
            worktree: &main,
            current: false,
        }];
        assert_eq!(
            plain(
                &rows,
                &[Column::Path, Column::Head, Column::Branch, Column::State],
                8
            ),
            "/repo\t14b96db3\tmain\t\n"
        );
    }

    #[test]
    fn plain_marks_current_in_state_and_blanks_detached() {
        let detached = worktree("/repo", None);
        let rows = [Row {
            worktree: &detached,
            current: true,
        }];
        assert_eq!(
            plain(
                &rows,
                &[Column::Path, Column::Head, Column::Branch, Column::State],
                8
            ),
            "/repo\t14b96db3\t\tcurrent\n"
        );
    }

    #[test]
    fn state_joins_every_flag_in_order() {
        let all = flagged("/repo");
        let rows = [Row {
            worktree: &all,
            current: true,
        }];
        assert_eq!(
            plain(&rows, &[Column::State], 8),
            "current bare locked prunable\n"
        );
        assert_eq!(
            table(&rows, &[Column::State], 8, None),
            "  STATE\n* bare locked prunable\n"
        );
    }

    #[test]
    fn plain_honors_head_length_and_column_order() {
        let main = worktree("/repo", Some("main"));
        let rows = [Row {
            worktree: &main,
            current: false,
        }];
        assert_eq!(
            plain(&rows, &[Column::Branch, Column::Head], 12),
            "main\t14b96db3c138\n"
        );
    }

    #[test]
    fn table_has_header_marker_and_aligned_columns() {
        let a = worktree("/work/alpha", Some("main"));
        let b = worktree("/work/b", Some("feature/long-name"));
        let rows = [
            Row {
                worktree: &a,
                current: true,
            },
            Row {
                worktree: &b,
                current: false,
            },
        ];
        assert_eq!(
            table(&rows, &ALL_COLUMNS, 8, None),
            "  NAME   BRANCH             HEAD      STATE  PATH\n\
             * alpha  main               14b96db3         /work/alpha\n\
             \x20 b      feature/long-name  14b96db3         /work/b\n"
        );
    }

    #[test]
    fn table_shortens_home_and_names_the_last_component() {
        let inside = worktree("/Users/me/code/w3", Some("main"));
        let outside = worktree("/srv/other", None);
        let rows = [
            Row {
                worktree: &inside,
                current: false,
            },
            Row {
                worktree: &outside,
                current: false,
            },
        ];
        assert_eq!(
            table(
                &rows,
                &[Column::Name, Column::Path],
                8,
                Some(Path::new("/Users/me"))
            ),
            "  NAME   PATH\n\
             \x20 w3     ~/code/w3\n\
             \x20 other  /srv/other\n"
        );
    }

    #[test]
    fn table_names_a_root_path_by_itself() {
        let root = worktree("/", Some("main"));
        let rows = [Row {
            worktree: &root,
            current: false,
        }];
        assert_eq!(table(&rows, &[Column::Name], 8, None), "  NAME\n\x20 /\n");
    }

    #[test]
    fn table_widths_count_characters_not_bytes() {
        let umlaut = worktree("/äöü", Some("main"));
        let ascii = worktree("/ab", Some("dev"));
        let rows = [
            Row {
                worktree: &umlaut,
                current: false,
            },
            Row {
                worktree: &ascii,
                current: false,
            },
        ];
        assert_eq!(
            table(&rows, &[Column::Path, Column::Branch], 8, None),
            "  PATH  BRANCH\n\x20 /äöü  main\n\x20 /ab   dev\n"
        );
    }

    #[test]
    fn empty_table_is_the_header_and_empty_plain_is_nothing() {
        assert_eq!(table(&[], &[Column::Name], 8, None), "  NAME\n");
        assert_eq!(plain(&[], &[Column::Name], 8), "");
    }

    #[test]
    fn json_keeps_field_order_full_sha_and_nulls() {
        let detached = worktree("/repo", None);
        let rows = [Row {
            worktree: &detached,
            current: true,
        }];
        assert_eq!(
            json(&rows, &ALL_FIELDS),
            format!(
                "[{{\"path\":\"/repo\",\"head\":\"{HEAD}\",\"branch\":null,\"bare\":false,\"locked\":null,\"prunable\":null,\"current\":true}}]\n"
            )
        );
    }

    #[test]
    fn json_filters_fields_and_carries_reasons() {
        let all = flagged("/repo");
        let rows = [Row {
            worktree: &all,
            current: false,
        }];
        assert_eq!(
            json(&rows, &[Field::Prunable, Field::Locked, Field::Branch]),
            "[{\"prunable\":\"gitdir file points to non-existent location\",\"locked\":\"\",\"branch\":\"main\"}]\n"
        );
    }

    #[test]
    fn empty_json_is_an_empty_array() {
        assert_eq!(json(&[], &ALL_FIELDS), "[]\n");
    }
}
