use lazy_static::lazy_static;
use regex::Regex;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse, Position};

use crate::api_manager::ApiManager;

lazy_static! {
    // Matches require*(**.React) where * is any number of white space and ** is any number of characters
    static ref REACT_PATTERN: Regex = Regex::new(r#"require\s*\(\s*[^)]*\.React\s*\)"#).unwrap();
    static ref REACT_VAR_PATTERN: Regex = Regex::new(
        r#"(?i)\b(?:local\s+)?(\w+)\s*=\s*require\s*\(.*\.React\s*\)"#
    ).unwrap();
    static ref FIRST_QUOTES_PATTERN: Regex = Regex::new(r#""(.+)""#).unwrap();
}

fn has_react(doc: &str) -> bool {
    REACT_PATTERN.is_match(doc)
}

// TODO: This should return warning when multiple reacts are detected
// This method is used for checking what name was used for requiring react
fn get_react_var_name(doc: &str) -> Option<String> {
    if let Some(caps) = REACT_VAR_PATTERN.captures(doc) {
        return Some(caps.get(1).unwrap().as_str().to_string());
    }
    None
}

fn extract_name_from_span(span: &str) -> Option<String> {
    let args: Vec<&str> = span.split(',').collect();
    if let Some(first_arg) = args.get(0) {
        let trimmed = first_arg.trim();

        if trimmed.starts_with("[[") && trimmed.ends_with("]]") && trimmed.len() >= 4 {
            return Some(trimmed[2..trimmed.len() - 2].to_string());
        }

        if trimmed.len() >= 2 {
            let first_char = trimmed.chars().next();
            let last_char = trimmed.chars().rev().next();
            if (first_char == Some('"') || first_char == Some('\'') || first_char == Some('`'))
                && first_char == last_char
            {
                return Some(trimmed[1..trimmed.len() - 1].to_string());
            }
        }
    }
    None
}

fn get_instance_property_diagnostics(
    instance_name: &str,
    api_manager: &ApiManager,
) -> Vec<CompletionItem> {
    let mut diagnostics: Vec<CompletionItem> = Vec::new();

    if let Some(parsed_instance) = api_manager.lookup_inst(instance_name) {
        for property in &parsed_instance.properties {
            diagnostics.push(CompletionItem {
                label: property.name.clone(),
                kind: Some(CompletionItemKind::FIELD),
                detail: Some(property.data_type.clone()),
                ..Default::default()
            });
        }
    }

    diagnostics
}

fn get_instance_names(api_manager: &ApiManager) -> Vec<CompletionItem> {
    let mut diagnostics: Vec<CompletionItem> = Vec::new();

    if let Some(inst_names) = api_manager.get_all_inst() {
        for property in &inst_names {
            diagnostics.push(CompletionItem {
                label: property.clone(),
                kind: Some(CompletionItemKind::CLASS),
                ..Default::default()
            });
        }
    }

    diagnostics
}

fn position_to_byte_offset(doc: &str, position: &Position) -> Option<usize> {
    let mut byte_offset = 0;

    for (line_index, line) in doc.split_inclusive('\n').enumerate() {
        if line_index == position.line as usize {
            let mut utf16_units = 0;

            for (byte_index, ch) in line.char_indices() {
                if utf16_units >= position.character as usize {
                    return Some(byte_offset + byte_index);
                }
                utf16_units += ch.len_utf16();
            }

            return Some(byte_offset + line.len());
        }

        byte_offset += line.len();
    }

    None
}

fn context_is_assignment(doc: &str, cursor_byte_offset: usize) -> bool {
    if cursor_byte_offset > doc.len() {
        return false;
    }

    let bytes = doc.as_bytes();
    for i in (0..cursor_byte_offset).rev() {
        match bytes[i] {
            b'=' => return true,
            b'\n' => return false,
            b',' => return false,
            b';' => return false,
            _ => continue,
        }
    }
    false
}

fn is_cursor_in_context(byte_cursor: usize, region: &str, context: &Regex) -> bool {
    if let Some(caps) = context.captures(region) {
        for i in 1..caps.len() {
            if let Some(group) = caps.get(i) {
                let byte_start = group.start();
                let byte_end = group.end();

                if byte_cursor >= byte_start && byte_cursor <= byte_end {
                    return true;
                }
            }
        }
    }
    false
}

fn get_completion_items(
    doc: &str,
    cursor: &Position,
    api_manager: &ApiManager,
) -> Vec<CompletionItem> {
    let mut diagnostics: Vec<CompletionItem> = Vec::new();
    if !has_react(doc) {
        return diagnostics;
    }
    let variable_name = get_react_var_name(doc);
    if variable_name.is_none() {
        return diagnostics;
    }

    let cursor_byte_offset =
        position_to_byte_offset(doc, cursor).expect("Invalid position given for doc!");

    let create_element_pattern = format!(
        r#"(?s){}\.createElement\s*\((.*?)\)"#,
        variable_name.unwrap()
    );
    let rgx = Regex::new(&create_element_pattern).unwrap();

    for caps in rgx.captures_iter(doc) {
        if let Some(group) = caps.get(1) {
            let group_str = group.as_str();
            let local_cursor_offset = cursor_byte_offset - group.start();

            let brace_re = Regex::new(r"(?s)\{(.*?)\}").unwrap();
            if is_cursor_in_context(local_cursor_offset, group_str, &brace_re) {
                if !context_is_assignment(doc, cursor_byte_offset) {
                    if let Some(instance_name) = extract_name_from_span(group_str) {
                        let diags = get_instance_property_diagnostics(&instance_name, api_manager);
                        diagnostics.extend(diags);
                    }
                }

                break;
            }

            let quotes_re =
                Regex::new(r#"(?s)(?:"([^"]*?)"|'([^']*?)'|`([^`]*?)`|\[\[([^\]]*?)\]\])"#)
                    .unwrap();
            if is_cursor_in_context(local_cursor_offset, group_str, &quotes_re) {
                let diags = get_instance_names(api_manager);
                diagnostics.extend(diags);

                break;
            }
        }
    }

    diagnostics
}

pub fn generate_auto_completions(
    doc: &str,
    cursor: &Position,
    api_manager: &ApiManager,
) -> Result<CompletionResponse, Box<dyn std::error::Error>> {
    Ok(CompletionResponse::Array(get_completion_items(
        doc,
        cursor,
        api_manager,
    )))
}

#[cfg(test)]
mod tests {
    use crate::file_diagnoser::{extract_name_from_span, get_react_var_name};

    #[test]
    fn test_react_variable_name_search() {
        assert_eq!(
            get_react_var_name(r#"local Test = require(Somewhere.Somehow.Sometime.React);"#),
            Some("Test".to_string())
        );
        assert_eq!(
            get_react_var_name(r#"local Test = require(Somewhere.Somehow.Sometime.React)"#),
            Some("Test".to_string())
        );
        assert_eq!(
            get_react_var_name(r#"local _Best123 = require(Somewhere.Somehow.Sometime.React);"#),
            Some("_Best123".to_string())
        );
        assert_eq!(
            get_react_var_name(r#"local P = require(Test.React)"#),
            Some("P".to_string())
        );
    }

    #[test]
    fn test_instance_names() {
        assert_eq!(
            extract_name_from_span(r#"'Frame', { ... }"#),
            Some("Frame".to_string())
        );
        assert_eq!(
            extract_name_from_span(r#"`TextLabel`,\n { ["Test"] = "Huh", ... }"#),
            Some("TextLabel".to_string())
        );
        assert_eq!(
            extract_name_from_span(
                r#""UIPadding",
            {
                Text = "Wrong Answer"
            }"#
            ),
            Some("UIPadding".to_string())
        );
        assert_eq!(extract_name_from_span(r#"[Frame], { ... }"#), None);
        assert_eq!(
            extract_name_from_span(
                r#"{
            ["Test"] = "Wrong",
        }"#
            ),
            None
        );
        assert_eq!(extract_name_from_span(r#"{"Wrong"}"#), None);
    }
}
