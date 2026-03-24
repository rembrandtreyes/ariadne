use crate::parse::types::{
    Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind,
};
use crate::parse::LanguageParser;

pub struct GoParser;

impl Default for GoParser {
    fn default() -> Self {
        Self::new()
    }
}

impl GoParser {
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
                "function_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        }
                        .clone();
                        let is_exported = name.starts_with(|c: char| c.is_uppercase());
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Function,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported,
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
                        Self::extract_symbols(child, source, result, Some(&qualified));
                    }
                }
                "method_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let receiver = child
                            .child_by_field_name("receiver")
                            .and_then(|r| r.utf8_text(source.as_bytes()).ok())
                            .unwrap_or_default()
                            .to_string();
                        let is_exported = name.starts_with(|c: char| c.is_uppercase());
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: format!("{}.{}", receiver, name),
                            kind: SymbolKind::Method,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported,
                            signature: child
                                .utf8_text(source.as_bytes())
                                .unwrap_or_default()
                                .lines()
                                .next()
                                .unwrap_or_default()
                                .to_string(),
                            decorators: Vec::new(),
                            parent_name: Some(receiver.clone()),
                        });
                        Self::extract_symbols(
                            child,
                            source,
                            result,
                            Some(&format!("{}.{}", receiver, name)),
                        );
                    }
                }
                "type_declaration" => {
                    Self::extract_type_specs(child, source, result);
                }
                "const_declaration" => {
                    Self::extract_const_var_decls(child, source, result, SymbolKind::Constant);
                }
                "var_declaration" => {
                    Self::extract_const_var_decls(child, source, result, SymbolKind::Variable);
                }
                "import_declaration" => {
                    Self::extract_go_imports(child, source, result);
                }
                "call_expression" => {
                    if let Some(func_node) = child.child_by_field_name("function") {
                        let callee = func_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let caller = parent_name.unwrap_or("<package>").to_string();
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

    fn extract_type_specs(node: tree_sitter::Node, source: &str, result: &mut ParseResult) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = name_node
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    let is_exported = name.starts_with(|c: char| c.is_uppercase());
                    let kind = if child.child_by_field_name("type").map(|t| t.kind())
                        == Some("struct_type")
                    {
                        SymbolKind::Class
                    } else if child.child_by_field_name("type").map(|t| t.kind())
                        == Some("interface_type")
                    {
                        SymbolKind::Interface
                    } else {
                        SymbolKind::TypeAlias
                    };
                    result.symbols.push(ParsedSymbol {
                        name: name.clone(),
                        qualified_name: name.clone(),
                        kind,
                        line_start: child.start_position().row as u32 + 1,
                        line_end: child.end_position().row as u32 + 1,
                        is_exported,
                        signature: format!("type {}", name),
                        decorators: Vec::new(),
                        parent_name: None,
                    });
                }
            }
        }
    }

    fn extract_const_var_decls(
        node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
        kind: SymbolKind,
    ) {
        let spec_kind = if kind == SymbolKind::Constant {
            "const_spec"
        } else {
            "var_spec"
        };
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == spec_kind {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = name_node
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    let is_exported = name.starts_with(|c: char| c.is_uppercase());
                    let sig_prefix = if kind == SymbolKind::Constant {
                        "const"
                    } else {
                        "var"
                    };
                    result.symbols.push(ParsedSymbol {
                        name: name.clone(),
                        qualified_name: name.clone(),
                        kind,
                        line_start: child.start_position().row as u32 + 1,
                        line_end: child.end_position().row as u32 + 1,
                        is_exported,
                        signature: format!("{} {}", sig_prefix, name),
                        decorators: Vec::new(),
                        parent_name: None,
                    });
                }
            }
        }
    }

    fn extract_go_imports(node: tree_sitter::Node, source: &str, result: &mut ParseResult) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "import_spec" {
                // import_spec can have an optional alias (name field) and a path field
                let path_node = child.child_by_field_name("path");
                let alias_node = child.child_by_field_name("name");

                let path_text = path_node
                    .and_then(|p| p.utf8_text(source.as_bytes()).ok())
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string();

                if path_text.is_empty() {
                    continue;
                }

                // Determine imported_name: alias if present, otherwise last path component
                let imported_name = if let Some(alias) = alias_node {
                    let alias_text = alias
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    if alias_text == "." || alias_text == "_" {
                        // Dot import or blank import: use last path component
                        path_text
                            .split('/')
                            .next_back()
                            .unwrap_or_default()
                            .to_string()
                    } else {
                        alias_text
                    }
                } else {
                    path_text
                        .split('/')
                        .next_back()
                        .unwrap_or_default()
                        .to_string()
                };

                // All Go imports are external (stdlib or third-party packages)
                // Internal imports would need go.mod module path context to detect
                let is_external = true;

                result.imports.push(ParsedImport {
                    imported_name,
                    module_path: path_text,
                    line: child.start_position().row as u32 + 1,
                    is_external,
                });
            } else if child.kind() == "interpreted_string_literal" {
                // Single import without import_spec wrapper (e.g., import "fmt")
                let text = child
                    .utf8_text(source.as_bytes())
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string();
                if !text.is_empty() {
                    result.imports.push(ParsedImport {
                        imported_name: text.split('/').next_back().unwrap_or_default().to_string(),
                        module_path: text,
                        line: child.start_position().row as u32 + 1,
                        is_external: true,
                    });
                }
            } else if child.kind() == "import_spec_list" {
                Self::extract_go_imports(child, source, result);
            }
        }
    }
}

impl LanguageParser for GoParser {
    fn language(&self) -> Language {
        Language::Go
    }

    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into())?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

        let root = tree.root_node();
        let mut result = ParseResult::default();

        Self::extract_symbols(root, source, &mut result, None);

        Ok(result)
    }
}
