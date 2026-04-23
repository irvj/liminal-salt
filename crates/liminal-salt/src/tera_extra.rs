//! Tera filter + helper registration, matching the Django template filters that
//! templates rely on (`|markdown`, `|display_name`).

use std::collections::HashMap;

use pulldown_cmark::{Options, Parser, html};
use serde_json::Value;
use tera::{Result, Tera};

/// Register every project-specific filter on a Tera instance.
pub fn register(tera: &mut Tera) {
    tera.register_filter("markdown", markdown_filter);
    tera.register_filter("display_name", display_name_filter);
    tera.register_filter("escapejs", escapejs_filter);
}

fn markdown_filter(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("markdown filter expects a string"))?;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(input, options);
    let mut out = String::with_capacity(input.len() + input.len() / 4);
    html::push_html(&mut out, parser);
    Ok(Value::String(out))
}

/// `"assistant_role"` → `"Assistant Role"`. Matches the Django `display_name`
/// template filter used by sidebar persona group headings.
fn display_name_filter(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("display_name filter expects a string"))?;
    let pretty: String = input
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Value::String(pretty))
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Port of Django's `|escapejs` filter. Emits JS-string-literal-safe output
/// with HTML-dangerous chars also escaped as `\uXXXX` so the result is safe in
/// both `<script>` and attribute contexts.
fn escapejs_filter(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("escapejs filter expects a string"))?;
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\u005C"),
            '\'' => out.push_str("\\u0027"),
            '"' => out.push_str("\\u0022"),
            '>' => out.push_str("\\u003E"),
            '<' => out.push_str("\\u003C"),
            '&' => out.push_str("\\u0026"),
            '=' => out.push_str("\\u003D"),
            '-' => out.push_str("\\u002D"),
            ';' => out.push_str("\\u003B"),
            '`' => out.push_str("\\u0060"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    Ok(Value::String(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn apply(filter: fn(&Value, &HashMap<String, Value>) -> Result<Value>, v: Value) -> String {
        filter(&v, &HashMap::new()).unwrap().as_str().unwrap().to_string()
    }

    #[test]
    fn markdown_basic() {
        let out = apply(markdown_filter, json!("**bold** _italic_"));
        assert!(out.contains("<strong>bold</strong>"));
        assert!(out.contains("<em>italic</em>"));
    }

    #[test]
    fn markdown_tables() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |";
        let out = apply(markdown_filter, json!(md));
        assert!(out.contains("<table>"));
    }

    #[test]
    fn display_name_snake_to_title() {
        assert_eq!(apply(display_name_filter, json!("assistant_role")), "Assistant Role");
        assert_eq!(apply(display_name_filter, json!("my-persona")), "My Persona");
        assert_eq!(apply(display_name_filter, json!("single")), "Single");
        assert_eq!(apply(display_name_filter, json!("")), "");
    }

    #[test]
    fn escapejs_escapes_html_and_quotes() {
        assert_eq!(
            apply(escapejs_filter, json!("hello \"world\"")),
            "hello \\u0022world\\u0022"
        );
        assert_eq!(apply(escapejs_filter, json!("<script>")), "\\u003Cscript\\u003E");
        assert_eq!(apply(escapejs_filter, json!("a & b")), "a \\u0026 b");
        assert_eq!(apply(escapejs_filter, json!("plain text")), "plain text");
    }

    #[test]
    fn escapejs_escapes_control_chars() {
        assert_eq!(apply(escapejs_filter, json!("line1\nline2")), "line1\\u000Aline2");
        assert_eq!(apply(escapejs_filter, json!("a\tb")), "a\\u0009b");
    }
}
