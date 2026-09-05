use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;

use crate::render::{Column, Field, parse_list};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Table,
    Plain,
    Json,
}

impl FromStr for Format {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "table" => Ok(Format::Table),
            "plain" => Ok(Format::Plain),
            "json" => Ok(Format::Json),
            other => Err(format!("unknown format: {other}")),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Layer {
    pub format: Option<Format>,
    pub format_tty: Option<Format>,
    pub format_pipe: Option<Format>,
    pub head_length: Option<usize>,
    pub table_columns: Option<Vec<Column>>,
    pub plain_columns: Option<Vec<Column>>,
    pub columns: Option<Vec<Column>>,
    pub fields: Option<Vec<Field>>,
    pub add_path: Option<String>,
    pub add_include: Option<String>,
    pub add_base: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Settings {
    pub format: Option<Format>,
    pub format_tty: Format,
    pub format_pipe: Format,
    pub head_length: usize,
    pub table_columns: Vec<Column>,
    pub plain_columns: Vec<Column>,
    pub columns: Option<Vec<Column>>,
    pub fields: Vec<Field>,
    pub add_path: String,
    pub add_include: String,
    pub add_base: Option<String>,
}

impl Settings {
    pub fn mode(&self, is_terminal: bool) -> Format {
        self.format.unwrap_or(if is_terminal {
            self.format_tty
        } else {
            self.format_pipe
        })
    }

    pub fn columns_for(&self, mode: Format) -> &[Column] {
        self.columns.as_deref().unwrap_or(match mode {
            Format::Table => &self.table_columns,
            Format::Plain | Format::Json => &self.plain_columns,
        })
    }
}

pub fn resolve(layers: &[Layer]) -> Settings {
    let mut settings = Settings {
        format: None,
        format_tty: Format::Table,
        format_pipe: Format::Plain,
        head_length: 8,
        table_columns: vec![
            Column::Name,
            Column::Branch,
            Column::Head,
            Column::State,
            Column::Path,
        ],
        plain_columns: vec![Column::Path, Column::Head, Column::Branch, Column::State],
        columns: None,
        fields: vec![
            Field::Path,
            Field::Head,
            Field::Branch,
            Field::Bare,
            Field::Locked,
            Field::Prunable,
            Field::Current,
        ],
        add_path: "~/.worktrees/{repo}/{name}".to_string(),
        add_include: ".worktreeinclude".to_string(),
        add_base: None,
    };
    for layer in layers {
        if let Some(format) = layer.format {
            settings.format = Some(format);
        }
        if let Some(format) = layer.format_tty {
            settings.format_tty = format;
        }
        if let Some(format) = layer.format_pipe {
            settings.format_pipe = format;
        }
        if let Some(length) = layer.head_length {
            settings.head_length = length;
        }
        if let Some(columns) = &layer.table_columns {
            settings.table_columns = columns.clone();
        }
        if let Some(columns) = &layer.plain_columns {
            settings.plain_columns = columns.clone();
        }
        if let Some(columns) = &layer.columns {
            settings.columns = Some(columns.clone());
        }
        if let Some(fields) = &layer.fields {
            settings.fields = fields.clone();
        }
        if let Some(path) = &layer.add_path {
            settings.add_path = path.clone();
        }
        if let Some(include) = &layer.add_include {
            settings.add_include = include.clone();
        }
        if let Some(base) = &layer.add_base {
            settings.add_base = Some(base.clone());
        }
    }
    settings
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    head_length: Option<usize>,
    format: Option<FormatSection>,
    table: Option<ColumnsSection>,
    plain: Option<ColumnsSection>,
    json: Option<FieldsSection>,
    add: Option<AddSection>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct AddSection {
    path: Option<String>,
    include: Option<String>,
    base: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FormatSection {
    tty: Option<Format>,
    pipe: Option<Format>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColumnsSection {
    columns: Option<Vec<Column>>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldsSection {
    fields: Option<Vec<Field>>,
}

impl FileConfig {
    fn into_layer(self) -> Result<Layer, String> {
        let format = self.format.unwrap_or_default();
        let add = self.add.unwrap_or_default();
        Ok(Layer {
            format: None,
            format_tty: format.tty,
            format_pipe: format.pipe,
            head_length: self.head_length.map(head_length).transpose()?,
            table_columns: self.table.and_then(|section| section.columns),
            plain_columns: self.plain.and_then(|section| section.columns),
            columns: None,
            fields: self.json.and_then(|section| section.fields),
            add_path: add.path,
            add_include: add.include,
            add_base: add.base,
        })
    }
}

const HEAD_LENGTH_RANGE: &str = "head_length must be 1 to 40";

fn head_length(value: usize) -> Result<usize, String> {
    if (1..=40).contains(&value) {
        Ok(value)
    } else {
        Err(HEAD_LENGTH_RANGE.to_string())
    }
}

pub fn user_file(xdg_config_home: Option<&str>, home: Option<&Path>) -> Option<PathBuf> {
    match xdg_config_home.filter(|dir| !dir.is_empty()) {
        Some(dir) => Some(Path::new(dir).join("w3/config.toml")),
        None => home.map(|home| home.join(".config/w3/config.toml")),
    }
}

pub fn repo_file(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .map(|dir| dir.join("az.toml"))
        .find(|path| path.is_file())
}

pub fn load_user_file(path: &Path) -> Result<Layer, String> {
    let Some(text) = read(path)? else {
        return Ok(Layer::default());
    };
    let config: FileConfig = toml::from_str(&text).map_err(|error| at(path, error.message()))?;
    config.into_layer().map_err(|error| at(path, &error))
}

pub fn load_repo_file(path: &Path) -> Result<Layer, String> {
    let Some(text) = read(path)? else {
        return Ok(Layer::default());
    };
    let mut table: toml::Table =
        toml::from_str(&text).map_err(|error| at(path, error.message()))?;
    let Some(section) = table.remove("w3") else {
        return Ok(Layer::default());
    };
    let config: FileConfig = section
        .try_into()
        .map_err(|error: toml::de::Error| at(path, error.message()))?;
    config.into_layer().map_err(|error| at(path, &error))
}

fn read(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(at(path, &error.to_string())),
    }
}

fn at(path: &Path, message: &str) -> String {
    format!("{}: {message}", path.display())
}

pub fn from_env(var: impl Fn(&str) -> Option<String>) -> Result<Layer, String> {
    let mut layer = Layer::default();
    if let Some(value) = var("W3_FORMAT") {
        layer.format = Some(
            value
                .parse()
                .map_err(|error| format!("W3_FORMAT: {error}"))?,
        );
    }
    if let Some(value) = var("W3_HEAD_LENGTH") {
        let length = value
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|length| head_length(length).ok())
            .ok_or_else(|| format!("W3_HEAD_LENGTH: {HEAD_LENGTH_RANGE}"))?;
        layer.head_length = Some(length);
    }
    if let Some(value) = var("W3_COLUMNS") {
        layer.columns = Some(parse_list(&value).map_err(|error| format!("W3_COLUMNS: {error}"))?);
    }
    if let Some(value) = var("W3_FIELDS") {
        layer.fields = Some(parse_list(&value).map_err(|error| format!("W3_FIELDS: {error}"))?);
    }
    layer.add_path = var("W3_ADD_PATH");
    layer.add_include = var("W3_ADD_INCLUDE");
    layer.add_base = var("W3_ADD_BASE");
    Ok(layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
head_length = 12

[format]
tty = "plain"
pipe = "json"

[table]
columns = ["name", "path"]

[plain]
columns = ["path"]

[json]
fields = ["path", "current"]

[add]
path = "~/wt/{repo}/{name}"
include = ".w3include"
base = "origin/main"
"#;

    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn full_layer() -> Layer {
        Layer {
            format: None,
            format_tty: Some(Format::Plain),
            format_pipe: Some(Format::Json),
            head_length: Some(12),
            table_columns: Some(vec![Column::Name, Column::Path]),
            plain_columns: Some(vec![Column::Path]),
            columns: None,
            fields: Some(vec![Field::Path, Field::Current]),
            add_path: Some("~/wt/{repo}/{name}".to_string()),
            add_include: Some(".w3include".to_string()),
            add_base: Some("origin/main".to_string()),
        }
    }

    #[test]
    fn format_names_round_trip_and_reject_unknown() {
        assert_eq!("table".parse::<Format>(), Ok(Format::Table));
        assert_eq!("plain".parse::<Format>(), Ok(Format::Plain));
        assert_eq!("json".parse::<Format>(), Ok(Format::Json));
        assert_eq!(
            "yaml".parse::<Format>(),
            Err("unknown format: yaml".to_string())
        );
    }

    #[test]
    fn user_file_reads_every_knob() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write(tmp.path(), "config.toml", FULL);
        assert_eq!(load_user_file(&path), Ok(full_layer()));
    }

    #[test]
    fn repo_file_reads_only_the_w3_table() {
        let tmp = tempfile::tempdir().unwrap();
        let text = format!(
            "[project]\nname = \"x\"\n\n[workflow]\nbranch = \"direct\"\n\n[w3]\n{}",
            FULL.replacen("[format]", "[w3.format]", 1)
                .replacen("[table]", "[w3.table]", 1)
                .replacen("[plain]", "[w3.plain]", 1)
                .replacen("[json]", "[w3.json]", 1)
                .replacen("[add]", "[w3.add]", 1)
        );
        let path = write(tmp.path(), "az.toml", &text);
        assert_eq!(load_repo_file(&path), Ok(full_layer()));
    }

    #[test]
    fn repo_file_without_a_w3_table_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write(tmp.path(), "az.toml", "[project]\nname = \"x\"\n");
        assert_eq!(load_repo_file(&path), Ok(Layer::default()));
    }

    #[test]
    fn a_missing_file_is_an_empty_layer() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            load_user_file(&tmp.path().join("none.toml")),
            Ok(Layer::default())
        );
    }

    #[test]
    fn an_unknown_key_names_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write(tmp.path(), "config.toml", "head_len = 3\n");
        let error = load_user_file(&path).unwrap_err();
        assert!(error.starts_with(&path.display().to_string()), "{error}");
        assert!(error.contains("head_len"), "{error}");
    }

    #[test]
    fn an_unknown_column_in_a_file_names_the_file_and_the_column() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write(tmp.path(), "config.toml", "[table]\ncolumns = [\"nope\"]\n");
        let error = load_user_file(&path).unwrap_err();
        assert!(error.starts_with(&path.display().to_string()), "{error}");
        assert!(error.contains("unknown column: nope"), "{error}");
    }

    #[test]
    fn head_length_out_of_range_is_rejected_with_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        for text in ["head_length = 0\n", "head_length = 41\n"] {
            let path = write(tmp.path(), "config.toml", text);
            let error = load_user_file(&path).unwrap_err();
            assert!(error.starts_with(&path.display().to_string()), "{error}");
            assert!(error.contains("head_length must be 1 to 40"), "{error}");
        }
    }

    #[test]
    fn env_reads_every_variable() {
        let layer = from_env(|name| match name {
            "W3_FORMAT" => Some("json".into()),
            "W3_HEAD_LENGTH" => Some("40".into()),
            "W3_COLUMNS" => Some("head, name".into()),
            "W3_FIELDS" => Some("current".into()),
            "W3_ADD_PATH" => Some("/tmp/{name}".into()),
            "W3_ADD_INCLUDE" => Some(String::new()),
            "W3_ADD_BASE" => Some("main".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            layer,
            Layer {
                format: Some(Format::Json),
                head_length: Some(40),
                columns: Some(vec![Column::Head, Column::Name]),
                fields: Some(vec![Field::Current]),
                add_path: Some("/tmp/{name}".into()),
                add_include: Some(String::new()),
                add_base: Some("main".into()),
                ..Layer::default()
            }
        );
    }

    #[test]
    fn env_errors_name_the_variable() {
        let bad_columns = from_env(|name| (name == "W3_COLUMNS").then(|| "nope".to_string()));
        assert_eq!(
            bad_columns,
            Err("W3_COLUMNS: unknown column: nope".to_string())
        );
        let bad_length = from_env(|name| (name == "W3_HEAD_LENGTH").then(|| "0".to_string()));
        assert_eq!(
            bad_length,
            Err("W3_HEAD_LENGTH: head_length must be 1 to 40".to_string())
        );
        let not_a_number = from_env(|name| (name == "W3_HEAD_LENGTH").then(|| "x".to_string()));
        assert_eq!(
            not_a_number,
            Err("W3_HEAD_LENGTH: head_length must be 1 to 40".to_string())
        );
    }

    #[test]
    fn empty_env_is_an_empty_layer() {
        assert_eq!(from_env(|_| None), Ok(Layer::default()));
    }

    #[test]
    fn defaults_when_no_layer_sets_anything() {
        let settings = resolve(&[Layer::default()]);
        assert_eq!(settings.format, None);
        assert_eq!(settings.format_tty, Format::Table);
        assert_eq!(settings.format_pipe, Format::Plain);
        assert_eq!(settings.head_length, 8);
        assert_eq!(
            settings.table_columns,
            vec![
                Column::Name,
                Column::Branch,
                Column::Head,
                Column::State,
                Column::Path
            ]
        );
        assert_eq!(
            settings.plain_columns,
            vec![Column::Path, Column::Head, Column::Branch, Column::State]
        );
        assert_eq!(settings.columns, None);
        assert_eq!(settings.add_path, "~/.worktrees/{repo}/{name}");
        assert_eq!(settings.add_include, ".worktreeinclude");
        assert_eq!(settings.add_base, None);
        assert_eq!(
            settings.fields,
            vec![
                Field::Path,
                Field::Head,
                Field::Branch,
                Field::Bare,
                Field::Locked,
                Field::Prunable,
                Field::Current
            ]
        );
    }

    #[test]
    fn a_later_layer_wins_only_where_it_speaks() {
        let first = full_layer();
        let second = Layer {
            head_length: Some(4),
            format: Some(Format::Table),
            ..Layer::default()
        };
        let settings = resolve(&[first, second]);
        assert_eq!(settings.head_length, 4);
        assert_eq!(settings.format, Some(Format::Table));
        assert_eq!(settings.format_tty, Format::Plain);
        assert_eq!(settings.table_columns, vec![Column::Name, Column::Path]);
        assert_eq!(settings.add_path, "~/wt/{repo}/{name}");
        assert_eq!(settings.add_base.as_deref(), Some("origin/main"));
    }

    #[test]
    fn an_empty_include_at_a_later_layer_disables_copying() {
        let second = Layer {
            add_include: Some(String::new()),
            ..Layer::default()
        };
        let settings = resolve(&[full_layer(), second]);
        assert_eq!(settings.add_include, "");
    }

    #[test]
    fn mode_prefers_the_forced_format_then_the_terminal() {
        let mut settings = resolve(&[full_layer()]);
        assert_eq!(settings.mode(true), Format::Plain);
        assert_eq!(settings.mode(false), Format::Json);
        settings.format = Some(Format::Table);
        assert_eq!(settings.mode(true), Format::Table);
        assert_eq!(settings.mode(false), Format::Table);
    }

    #[test]
    fn columns_for_the_active_mode_prefer_the_override() {
        let mut settings = resolve(&[full_layer()]);
        assert_eq!(
            settings.columns_for(Format::Table),
            &[Column::Name, Column::Path]
        );
        assert_eq!(settings.columns_for(Format::Plain), &[Column::Path]);
        settings.columns = Some(vec![Column::Head]);
        assert_eq!(settings.columns_for(Format::Table), &[Column::Head]);
        assert_eq!(settings.columns_for(Format::Plain), &[Column::Head]);
    }

    #[test]
    fn user_file_follows_xdg_then_home() {
        assert_eq!(
            user_file(Some("/xdg"), Some(Path::new("/home/me"))),
            Some(PathBuf::from("/xdg/w3/config.toml"))
        );
        assert_eq!(
            user_file(None, Some(Path::new("/home/me"))),
            Some(PathBuf::from("/home/me/.config/w3/config.toml"))
        );
        assert_eq!(user_file(None, None), None);
    }

    #[test]
    fn repo_file_walks_up_and_stops_at_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        let deep = tmp.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(repo_file(&deep), None);
        let path = write(&tmp.path().join("a"), "az.toml", "");
        assert_eq!(repo_file(&deep), Some(path));
    }
}
