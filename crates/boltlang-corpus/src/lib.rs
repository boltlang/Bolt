//! The format of the test corpora in `tests/` at the root of the
//! repository, like `tests/checker` and `tests/run`. It's the format of the
//! tree-sitter corpus:
//!
//! ```text
//! ================================================================================
//! Mutual recursion
//! :todo
//! ================================================================================
//! fn even(n) = if n == 0 { True } else { odd(n - 1) };
//! --------------------------------------------------------------------------------
//! even: fn(i32) -> Bool
//! ```
//!
//! A case is a title between two lines of `=`, its input, and after each
//! line of `-` a section of output. Separators are at least three
//! characters long and start at the beginning of a line. Lines starting
//! with `:` after the title are attributes, which each test harness
//! interprets. A new case can leave out its output, and a harness fills it
//! in when it runs with `UPDATE_EXPECT=1`.
//!
//! The input can have several files, each starting with a line like
//! `//- /lib.bolt`: see [`split_files`].

use std::{
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
};

/// One case of a data file.
pub struct Case {
    pub title: String,
    /// The attribute lines, without their `:`, like `todo` or `todo wasm`.
    pub attributes: Vec<String>,
    pub input: String,
    /// The output sections, trimmed. Empty when the case has no output yet.
    pub sections: Vec<String>,
    /// The lines of the title and the input, which an update keeps.
    head: Range<usize>,
    /// All the lines of the case.
    lines: Range<usize>,
}

impl Case {
    /// Output section `n`, or `""` when there isn't one.
    pub fn section(&self, n: usize) -> &str {
        self.sections.get(n).map_or("", String::as_str)
    }
}

/// The line between the sections of a case.
pub const SECTION: &str = "--------------------------------------------------------------------------------";

/// Whether `line` is made of at least three `c`s.
fn is_separator(line: &str, c: char) -> bool {
    let line = line.trim_end();
    line.len() >= 3 && line.chars().all(|x| x == c)
}

/// The cases in the text of a data file, which have at most `max_sections`
/// sections of output.
pub fn parse_cases(text: &str, max_sections: usize) -> Result<Vec<Case>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cases = Vec::new();
    let mut i = 0;
    // Skip anything before the first case, like blank lines.
    while i < lines.len() && !is_separator(lines[i], '=') {
        i += 1;
    }
    while i < lines.len() {
        // The title, between two lines of `=`.
        let head_start = i;
        let start = i + 1;
        i = start;
        while i < lines.len() && !is_separator(lines[i], '=') {
            i += 1;
        }
        if i == lines.len() {
            return Err(format!("line {start}: the title isn't followed by a line of `=`"));
        }
        let (attribute_lines, title_lines): (Vec<&str>, Vec<&str>) =
            lines[start..i].iter().partition(|line| line.trim_start().starts_with(':'));
        let title = title_lines.join(" ").trim().to_string();
        let attributes = attribute_lines.iter().map(|line| line.trim()[1..].trim().to_string()).collect();
        i += 1;
        // The input and the sections, up to the next case.
        let mut parts = vec![Vec::new()];
        let mut head_end = None;
        while i < lines.len() && !is_separator(lines[i], '=') {
            if is_separator(lines[i], '-') {
                head_end.get_or_insert(i);
                parts.push(Vec::new());
            } else {
                parts.last_mut().unwrap().push(lines[i]);
            }
            i += 1;
        }
        if parts.len() > max_sections + 1 {
            let n = if max_sections == 1 { "one section".to_string() } else { format!("{max_sections} sections") };
            return Err(format!("`{title}`: expected the input and at most {n} of output"));
        }
        let mut parts = parts.into_iter().map(|part| part.join("\n").trim().to_string());
        let input = parts.next().unwrap_or_default() + "\n";
        let sections = parts.collect();
        let head = head_start..head_end.unwrap_or(i);
        cases.push(Case { title, attributes, input, sections, head, lines: head_start..i });
    }
    Ok(cases)
}

/// The test name for a title: `Mutual recursion` is `mutual_recursion`.
pub fn slug(title: &str) -> String {
    let slug: String = title.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    slug.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_")
}

/// The files of a case: its input split at the lines like `//- /lib.bolt`
/// that start a file, or `/main.bolt` without them.
pub fn split_files(input: &str) -> Result<Vec<(String, String)>, String> {
    if !input.lines().any(|line| line.starts_with("//-")) {
        return Ok(vec![("/main.bolt".to_string(), input.to_string())]);
    }
    let mut files: Vec<(String, String)> = Vec::new();
    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("//-") {
            let path = rest.trim();
            if !path.starts_with('/') || path.contains(char::is_whitespace) {
                return Err(format!("expected an absolute path after `//-`, like `//- /lib.bolt`: `{line}`"));
            }
            if files.iter().any(|(p, _)| p == path) {
                return Err(format!("two files are called `{path}`"));
            }
            files.push((path.to_string(), String::new()));
        } else if let Some((_, text)) = files.last_mut() {
            text.push_str(line);
            text.push('\n');
        } else if !line.trim().is_empty() {
            return Err(format!("code before the first `//-` line: `{line}`"));
        }
    }
    Ok(files)
}

/// Output sections as they're written in a data file, without the line of
/// `-` before the first. Empty sections at the end are left out.
pub fn show(sections: &[String]) -> String {
    let end = sections.iter().rposition(|s| !s.is_empty()).map_or(1, |i| i + 1);
    let mut out = String::new();
    for (i, section) in sections.iter().chain(std::iter::repeat(&String::new())).take(end).enumerate() {
        if i > 0 {
            out.push_str(SECTION);
            out.push('\n');
        }
        out.push_str(section);
        out.push('\n');
    }
    out
}

/// Write new output into the cases of `text`. `found` gives a case's
/// sections, or `None` to keep the case as it is, like one whose output is
/// written by hand.
pub fn update(text: &str, cases: &[Case], mut found: impl FnMut(&Case) -> Option<Vec<String>>) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    let first = cases.first().map_or(lines.len(), |c| c.head.start);
    for line in &lines[..first] {
        out.push_str(line);
        out.push('\n');
    }
    for (i, case) in cases.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let Some(sections) = found(case) else {
            let mut kept = &lines[case.lines.clone()];
            while let [rest @ .., last] = kept
                && last.trim().is_empty()
            {
                kept = rest;
            }
            for line in kept {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        };
        // Without the blank lines at the end of the input, so that the line
        // of `-` comes right after it.
        let mut head = &lines[case.head.clone()];
        while let [rest @ .., last] = head
            && last.trim().is_empty()
        {
            head = rest;
        }
        for line in head {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(SECTION);
        out.push('\n');
        out.push_str(&show(&sections));
    }
    out
}

/// Whether to write what the harness finds into the data files, instead
/// of comparing it.
pub fn updating() -> bool {
    std::env::var_os("UPDATE_EXPECT").is_some_and(|v| !v.is_empty() && v != "0")
}

/// The data files of `dir` and its subdirectories, in alphabetical order
/// of their paths.
pub fn data_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("can't read {}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(data_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "txt") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// The name of the data file `path` of `root` in test names: its path
/// without `.txt`, with `::` between directories, like `mutability::arrays`.
pub fn data_name(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path).with_extension("");
    relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("::")
}

/// What `f` gives, or the message it panicked with, after `what`.
pub fn catch<T>(what: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        Err(format!("{what} panicked: {message}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_puts_the_output_right_after_the_input() {
        let sep = "=".repeat(80);
        let text = format!("{sep}\nA case\n{sep}\nlet x = 1;\n\n\n{SECTION}\nold\n\n{sep}\nNo output yet\n{sep}\nx;\n\n");
        let cases = parse_cases(&text, 2).unwrap();
        let updated = update(&text, &cases, |case| Some(vec![format!("new {}", case.title)]));
        let expected = format!(
            "{sep}\nA case\n{sep}\nlet x = 1;\n{SECTION}\nnew A case\n\n{sep}\nNo output yet\n{sep}\nx;\n{SECTION}\nnew No output yet\n"
        );
        assert_eq!(updated, expected);
    }
}
