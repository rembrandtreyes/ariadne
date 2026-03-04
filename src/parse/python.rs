use crate::parse::types::{Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind};
use crate::parse::LanguageParser;

pub struct PythonParser;

impl PythonParser {
    pub fn new() -> Self {
        Self
    }

    fn extract_symbols(node: tree_sitter::Node, source: &str, result: &mut ParseResult, parent_name: Option<&str>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes()).unwrap_or_default().to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        };
                        let kind = if parent_name.is_some() {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified.clone(),
                            kind,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: !name.starts_with('_'),
                            signature: child.utf8_text(source.as_bytes())
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
                "class_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes()).unwrap_or_default().to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        };
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Class,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: !name.starts_with('_'),
                            signature: format!("class {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                        Self::extract_symbols(child, source, result, Some(&qualified));
                    }
                }
                "import_statement" => {
                    let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
                    let module = text.trim_start_matches("import ").trim().to_string();
                    result.imports.push(ParsedImport {
                        imported_name: module.clone(),
                        module_path: module,
                        line: child.start_position().row as u32 + 1,
                        is_external: true,
                    });
                }
                "import_from_statement" => {
                    let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
                    let module = if let Some(mod_node) = child.child_by_field_name("module_name") {
                        mod_node.utf8_text(source.as_bytes()).unwrap_or_default().to_string()
                    } else {
                        text.to_string()
                    };
                    let is_relative = text.contains("from .");
                    result.imports.push(ParsedImport {
                        imported_name: module.clone(),
                        module_path: module,
                        line: child.start_position().row as u32 + 1,
                        is_external: !is_relative,
                    });
                }
                "call" => {
                    if let Some(func_node) = child.child_by_field_name("function") {
                        let callee = func_node.utf8_text(source.as_bytes()).unwrap_or_default().to_string();
                        let caller = parent_name.unwrap_or("<module>").to_string();
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

impl LanguageParser for PythonParser {
    fn language(&self) -> Language {
        Language::Python
    }

    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_python::LANGUAGE.into())?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

        let root = tree.root_node();
        let mut result = ParseResult::default();

        Self::extract_symbols(root, source, &mut result, None);

        Ok(result)
    }
}
