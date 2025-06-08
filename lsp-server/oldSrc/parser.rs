use crate::metadata::get_metadata;
use lazy_static::lazy_static;
use regex::Regex;
use tower_lsp::lsp_types::{
    CodeDescription, CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity, Position,
    Range, Url,
};

lazy_static! {
    // Matches require*(**.React) where * is any number of white space and ** is any number of characters
    static ref REACT_PATTERN: Regex = Regex::new(r#"require\s*\(\s*[^)]*\.React\s*\)"#).unwrap();
    static ref REACT_VAR_PATTERN: Regex = Regex::new(
        r#"(?i)\b(?:local\s+)?(\w+)\s*=\s*require\s*\(.*\.React\s*\)"#
    ).unwrap();
}

fn has_react(text: &str) -> bool {
    return REACT_PATTERN.is_match(text);
}

fn extract_react_var_name(text: &str) -> Option<String> {
    if let Some(caps) = REACT_VAR_PATTERN.captures(text) {
        return Some(caps.get(1).unwrap().as_str().to_string());
    }
    None
}

fn extract_react_inst_name(args: &str) -> Option<String> {
    let args: Vec<&str> = args.split(',').collect();
    if let Some(first_arg) = args.get(0) {
        let trimmed = first_arg.trim();
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

fn pos_to_offset(text: &str, pos: Position) -> usize {
    let mut offset = 0;
    for (i, line) in text.lines().enumerate() {
        if i < pos.line as usize {
            offset += line.len() + 1;
        } else {
            offset += pos.character as usize;
            break;
        }
    }
    offset
}

fn parse_create_fns(react: &str, text: &str) {
    let create_element_pattern = format!(r#"(?s){}\.createElement\s*\((.*?)\)"#, react);
    let rgx = Regex::new(&create_element_pattern).unwrap();

    for caps in rgx.captures_iter(text) {
        if let Some(args) = caps.get(1) {
            let args_field = args.as_str();
            if let Some(inst_name) = extract_react_inst_name(args_field) {
                let properties = get_metadata(&inst_name);
            }
        }
    }
}

pub fn get_property_completions(text: &str, pos: Position) -> Vec<CompletionItem> {
    let offset = pos_to_offset(text, pos);
    let text_to_cursor = &text[..offset];

    // if create element doesn't even exist before cursor
    // Then no need to provide completions
    if let Some(crt_idx) = text_to_cursor.rfind(".createElement(") {
        // TODO: Check for quotes "" too to provide auto completion for instances
        if let Some(open_brace_rel_idx) = text_to_cursor[crt_idx..].find('{') {
            let abs_brace_idx = crt_idx + open_brace_rel_idx;
            // We are not inside brackets
            if offset < abs_brace_idx {
                return vec![];
            }

            let crt_call_txt = &text_to_cursor[crt_idx..];
            if let Some(par_start) = crt_call_txt.find('(') {
                let args_and_rest = &crt_call_txt[par_start + 1..];
                if let Some(comma_idx) = args_and_rest.find(',') {
                    let first_arg = &args_and_rest[..comma_idx];
                    if let Some(react_name) = extract_react_inst_name(first_arg) {
                        if let Some(properties) = get_metadata(&react_name) {
                            let completions: Vec<CompletionItem> = properties
                                .into_iter()
                                .map(|prop| CompletionItem {
                                    label: prop.name,
                                    kind: Some(CompletionItemKind::PROPERTY),
                                    ..Default::default()
                                })
                                .collect();
                            return completions;
                        }
                    }
                }
            }
        }
    }

    vec![CompletionItem {
        label: "Test".to_string(),
        kind: Some(CompletionItemKind::PROPERTY),
        ..Default::default()
    }]
}

pub fn parse_doc(uri: Url, text: &str) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    if !has_react(text) {
        return diagnostics;
    };
    if let Some(var_name) = extract_react_var_name(text) {
        parse_create_fns(&var_name, text);
    } else {
        let diag = Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: None,
            source: Some("parser".to_string()),
            message: "Found React require, but no variable name!".to_string(),
            code_description: Some(CodeDescription { href: uri }),
            data: None,
            related_information: None,
            tags: None,
        };
        diagnostics.push(diag);
    }

    return diagnostics;
}

#[cfg(test)]
mod tests {
    use crate::parser::extract_react_inst_name;
    use crate::parser::extract_react_var_name;
    use crate::parser::has_react;

    #[test]
    fn find_inst_name_in_args() {
        assert_eq!(
            extract_react_inst_name(r#"'Frame', { ... }"#),
            Some("Frame".to_string())
        );
        assert_eq!(
            extract_react_inst_name(r#"`TextLabel`,\n { ["Test"] = "Huh", ... }"#),
            Some("TextLabel".to_string())
        );
        assert_eq!(
            extract_react_inst_name(
                r#""UIPadding",
            {
                Text = "Wrong Answer"
            }"#
            ),
            Some("UIPadding".to_string())
        );
        assert_eq!(extract_react_inst_name(r#"[Frame], { ... }"#), None);
        assert_eq!(
            extract_react_inst_name(
                r#"{
            ["Test"] = "Wrong",
        }"#
            ),
            None
        );
        assert_eq!(extract_react_inst_name(r#"{"Wrong"}"#), None);
    }

    #[test]
    fn has_react_test() {
        assert!(has_react(
            r#"local React = require(ReplicatedStorage.Packages.React)"#
        ));
        assert!(has_react(r#"require(ReplicatedStorage.Packages.React)"#));
        assert!(has_react(r#"local Nothing = require(script.Parent.React)"#));
        assert!(!has_react(r#"require(script.Parent.NotReact)"#))
    }

    #[test]
    fn find_var_name() {
        assert_eq!(
            extract_react_var_name(r#"local React = require(ReplicatedStorage.Packages.React)"#),
            Some("React".to_string())
        );
        assert_eq!(
            extract_react_var_name(r#"require(ReplicatedStorage.Packages.React)"#),
            None
        );
        assert_eq!(
            extract_react_var_name(r#"local React = require(ReplicatedStorage.Packages.NotReact)"#),
            None
        )
    }
}
