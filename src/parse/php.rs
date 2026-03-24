use crate::parse::types::{
    Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind,
};
use crate::parse::LanguageParser;

pub struct PhpParser;

impl Default for PhpParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PhpParser {
    pub fn new() -> Self {
        Self
    }

    fn extract_symbols(
        node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
        parent_name: Option<&str>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}::{}", p, name),
                            None => name.clone(),
                        };
                        let kind = if parent_name.is_some() {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: true,
                            signature: child
                                .utf8_text(source.as_bytes())
                                .unwrap_or_default()
                                .lines()
                                .next()
                                .unwrap_or_default()
                                .to_string(),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                "method_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}::{}", p, name),
                            None => name.clone(),
                        };
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind: SymbolKind::Method,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: true,
                            signature: child
                                .utf8_text(source.as_bytes())
                                .unwrap_or_default()
                                .lines()
                                .next()
                                .unwrap_or_default()
                                .to_string(),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                "class_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}\\{}", p, name),
                            None => name.clone(),
                        };
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Class,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: true,
                            signature: format!("class {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                        Self::extract_symbols(child, source, result, Some(&qualified));
                    }
                }
                "interface_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}\\{}", p, name),
                            None => name.clone(),
                        };
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind: SymbolKind::Interface,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: true,
                            signature: format!("interface {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                "namespace_use_declaration" => {
                    let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
                    let module = text
                        .trim_start_matches("use ")
                        .trim_end_matches(';')
                        .trim()
                        .to_string();
                    result.imports.push(ParsedImport {
                        imported_name: module
                            .split('\\')
                            .next_back()
                            .unwrap_or_default()
                            .to_string(),
                        module_path: module,
                        line: child.start_position().row as u32 + 1,
                        is_external: true,
                    });
                }
                "trait_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}\\{}", p, name),
                            None => name.clone(),
                        };
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Trait,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: true,
                            signature: format!("trait {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                        Self::extract_symbols(child, source, result, Some(&qualified));
                    }
                }
                "object_creation_expression" => {
                    // PHP `new Foo()` — extract as a call to the class name
                    let mut class_name: Option<String> = None;
                    let mut oc_cursor = child.walk();
                    for oc_child in child.children(&mut oc_cursor) {
                        if oc_child.kind() == "name" || oc_child.kind() == "qualified_name" {
                            class_name = Some(
                                oc_child
                                    .utf8_text(source.as_bytes())
                                    .unwrap_or_default()
                                    .to_string(),
                            );
                            break;
                        }
                    }
                    if let Some(cls) = class_name {
                        let caller = parent_name.unwrap_or("<global>").to_string();
                        result.calls.push(ParsedCall {
                            caller_name: caller,
                            callee_name: cls,
                            line: child.start_position().row as u32 + 1,
                        });
                    }
                    // Also recurse into arguments for nested expressions
                    Self::extract_symbols(child, source, result, parent_name);
                }
                "function_call_expression"
                | "member_call_expression"
                | "scoped_call_expression" => {
                    if let Some(func_node) = child
                        .child_by_field_name("function")
                        .or_else(|| child.child_by_field_name("name"))
                    {
                        let callee = func_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let caller = parent_name.unwrap_or("<global>").to_string();
                        result.calls.push(ParsedCall {
                            caller_name: caller,
                            callee_name: callee,
                            line: child.start_position().row as u32 + 1,
                        });
                    }
                }
                _ => {
                    Self::extract_symbols(child, source, result, parent_name);
                }
            }
        }
    }
}

impl LanguageParser for PhpParser {
    fn language(&self) -> Language {
        Language::Php
    }

    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_php::LANGUAGE_PHP.into())?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

        let root = tree.root_node();
        let mut result = ParseResult::default();

        Self::extract_symbols(root, source, &mut result, None);

        Ok(result)
    }
}
