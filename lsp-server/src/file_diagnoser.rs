use lazy_static::lazy_static;
use lsp_positions::SpanCalculator;
use regex::Regex;
use tower_lsp::lsp_types::{CompletionItem, CompletionResponse, Position};

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
    if let Some(caps) = FIRST_QUOTES_PATTERN.captures(span) {
        return Some(caps.get(1).unwrap().as_str().to_string());
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
                ..Default::default()
            });
        }
    }

    diagnostics
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

    let mut sc = SpanCalculator::new(doc);
    let cursor_span = sc.for_line_and_column(cursor.line as usize, cursor.character as usize, 0);
    let cursor_byte_offset = cursor_span.containing_line.start + cursor_span.column.utf8_offset;

    let create_element_pattern = format!(
        r#"(?s){}\.createElement\s*\((.*?)\)"#,
        variable_name.unwrap()
    );
    let rgx = Regex::new(&create_element_pattern).unwrap();

    for caps in rgx.captures_iter(doc) {
        if let Some(group) = caps.get(1) {
            let byte_start = group.start();
            let byte_end = group.end();
            if cursor_byte_offset >= byte_start && cursor_byte_offset <= byte_end {
                if let Some(instance_name) = extract_name_from_span(group.as_str()) {
                    let diags = get_instance_property_diagnostics(&instance_name, api_manager);
                    diagnostics.extend(diags);
                }
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
