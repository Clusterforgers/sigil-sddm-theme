use std::fmt;
use std::path::{Path, PathBuf};

/// Why a figure file could not be used, said the way a compiler would: where, what, and
/// the offending line.
///
/// ```text
/// figure.json5:41:9: layers[2].elements[3]: unknown field `sidez`, expected one of …
///    41 |         { type: "polygon", sidez: 7, r: 318 },
///       |         ^
/// ```
#[derive(Debug)]
pub struct FigureError {
    file: Option<PathBuf>,
    /// 1-based line and column, when the parser knows them.
    position: Option<(usize, usize)>,
    /// Where in the figure it went wrong, e.g. `layers[2] "letter ring" › elements[3] (polygon)`.
    context: String,
    message: String,
    /// The source line at `position`, for the excerpt.
    excerpt: Option<String>,
}

impl FigureError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        FigureError { file: None, position: None, context: String::new(), message: message.into(), excerpt: None }
    }

    pub(super) fn file(mut self, path: &Path) -> Self {
        self.file = Some(path.to_path_buf());
        self
    }

    pub(super) fn context(mut self, context: impl Into<String>) -> Self {
        self.context = context.into();
        self
    }

    /// Point at 0-based `line` and `column` of `src`.
    pub(super) fn at(mut self, src: &str, line: usize, column: usize) -> Self {
        self.position = Some((line + 1, column + 1));
        self.excerpt = src.lines().nth(line).map(str::to_owned);
        self
    }

    /// A parse failure, with the field path serde was in and json5's position.
    pub(super) fn parse(src: &str, err: serde_path_to_error::Error<json5::Error>) -> Self {
        let path = err.path().to_string();
        let inner = err.into_inner();
        let mut message = inner.to_string();
        let mut e = FigureError::new("");
        if let Some(p) = inner.position() {
            // json5 appends the position to its message; we show it up front instead.
            if let Some(m) = message.strip_suffix(&format!(" at {p}")) {
                message = m.to_owned();
            }
            e = e.at(src, p.line, p.column);
        }
        e.message = message;
        if path != "." {
            e.context = path;
        }
        e
    }
}

impl fmt::Display for FigureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.file, self.position) {
            (Some(file), Some((line, col))) => write!(f, "{}:{line}:{col}: ", file.display())?,
            (Some(file), None) => write!(f, "{}: ", file.display())?,
            (None, Some((line, col))) => write!(f, "{line}:{col}: ")?,
            (None, None) => {}
        }
        if !self.context.is_empty() {
            write!(f, "{}: ", self.context)?;
        }
        write!(f, "{}", self.message)?;
        if let (Some((line, col)), Some(text)) = (self.position, &self.excerpt) {
            let gutter = line.to_string().len();
            write!(f, "\n {line} | {text}\n {:gutter$} | {:>col$}", "", "^")?;
        }
        Ok(())
    }
}

impl std::error::Error for FigureError {}
