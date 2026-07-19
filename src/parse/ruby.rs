use crate::parse::types::{
    Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind,
};
use crate::parse::LanguageParser;

pub struct RubyParser;

impl Default for RubyParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RubyParser {
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
                "method" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}#{}", p, name),
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
                "singleton_method" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        };
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind: SymbolKind::Method,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: true,
                            signature: format!("def self.{}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                "class" => {
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
                "module" => {
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
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Module,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: true,
                            signature: format!("module {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                        Self::extract_symbols(child, source, result, Some(&qualified));
                    }
                }
                "call" => {
                    if let Some(method_node) = child.child_by_field_name("method") {
                        let callee = method_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        if callee == "require" || callee == "require_relative" {
                            if let Some(args) = child.child_by_field_name("arguments") {
                                let text = args
                                    .utf8_text(source.as_bytes())
                                    .unwrap_or_default()
                                    .trim_matches('(')
                                    .trim_matches(')')
                                    .trim_matches('\'')
                                    .trim_matches('"')
                                    .to_string();
                                let is_relative = callee == "require_relative";
                                result.imports.push(ParsedImport {
                                    imported_name: text.clone(),
                                    module_path: text,
                                    line: child.start_position().row as u32 + 1,
                                    is_external: !is_relative,
                                    original_name: None,
                                });
                            }
                        } else if callee == "attr_accessor"
                            || callee == "attr_reader"
                            || callee == "attr_writer"
                        {
                            // attr_accessor/attr_reader/attr_writer generate methods
                            // Extract each symbol argument as a Method symbol
                            if let Some(args) = child.child_by_field_name("arguments") {
                                let mut arg_cursor = args.walk();
                                for arg in args.children(&mut arg_cursor) {
                                    if arg.kind() == "simple_symbol" {
                                        let sym_text = arg
                                            .utf8_text(source.as_bytes())
                                            .unwrap_or_default()
                                            .trim_start_matches(':')
                                            .to_string();
                                        if sym_text.is_empty() {
                                            continue;
                                        }
                                        let qualified = match parent_name {
                                            Some(p) => format!("{}#{}", p, sym_text),
                                            None => sym_text.clone(),
                                        };
                                        result.symbols.push(ParsedSymbol {
                                            name: sym_text.clone(),
                                            qualified_name: qualified,
                                            kind: SymbolKind::Method,
                                            line_start: child.start_position().row as u32 + 1,
                                            line_end: child.end_position().row as u32 + 1,
                                            is_exported: true,
                                            signature: format!("{} :{}", callee, sym_text),
                                            decorators: Vec::new(),
                                            parent_name: parent_name.map(String::from),
                                        });
                                    }
                                }
                            }
                        } else {
                            let caller = parent_name.unwrap_or("<main>").to_string();
                            result.calls.push(ParsedCall {
                                caller_name: caller,
                                callee_name: callee,
                                line: child.start_position().row as u32 + 1,
                            });
                        }
                    }
                }
                _ => {
                    Self::extract_symbols(child, source, result, parent_name);
                }
            }
        }
    }
}

impl LanguageParser for RubyParser {
    fn language(&self) -> Language {
        Language::Ruby
    }

    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_ruby::LANGUAGE.into())?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

        let root = tree.root_node();
        let mut result = ParseResult {
            syntax_error_count: crate::parse::count_syntax_errors(root),
            ..ParseResult::default()
        };

        Self::extract_symbols(root, source, &mut result, None);

        Ok(result)
    }
}
