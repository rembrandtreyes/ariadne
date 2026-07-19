use crate::parse::types::{
    Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind,
};
use crate::parse::LanguageParser;

pub struct PythonParser;

impl Default for PythonParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract decorator names from a `decorated_definition` node.
    /// Returns a vec of decorator strings (e.g. ["staticmethod", "app.route(\"/\")"]).
    fn extract_decorators(node: tree_sitter::Node, source: &str) -> Vec<String> {
        let mut decorators = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "decorator" {
                // The decorator node text includes the '@' prefix; strip it.
                let text = child
                    .utf8_text(source.as_bytes())
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let cleaned = text.strip_prefix('@').unwrap_or(&text).to_string();
                decorators.push(cleaned);
            }
        }
        decorators
    }

    /// Walk individual imported names from an `import_from_statement` node.
    /// For `from foo import bar, baz as q`, yields ("bar", "foo") and ("baz", "foo").
    fn extract_from_imports(node: tree_sitter::Node, source: &str) -> Vec<ParsedImport> {
        let line = node.start_position().row as u32 + 1;

        // Find the module path: first `dotted_name` or `relative_import` child.
        let mut module_path = String::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "dotted_name" | "relative_import" => {
                    if module_path.is_empty() {
                        module_path = child
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                    }
                    // After the first dotted_name (the module), subsequent dotted_name
                    // nodes are imported names -- handled below.
                    break;
                }
                _ => {}
            }
        }

        let is_relative = module_path.starts_with('.');

        // Collect imported names. They appear after the "import" keyword node.
        // They can be `dotted_name`, `aliased_import`, or `identifier` nodes.
        let mut imports = Vec::new();
        let mut past_import_keyword = false;
        let mut cursor2 = node.walk();
        for child in node.children(&mut cursor2) {
            if child.kind() == "import" {
                past_import_keyword = true;
                continue;
            }
            if !past_import_keyword {
                continue;
            }

            match child.kind() {
                "dotted_name" | "identifier" => {
                    let name = child
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    imports.push(ParsedImport {
                        imported_name: name,
                        module_path: module_path.clone(),
                        line,
                        is_external: !is_relative,
                        original_name: None,
                    });
                }
                "aliased_import" => {
                    // `aliased_import` has a `name` field and an optional `alias` field.
                    let name = if let Some(name_node) = child.child_by_field_name("name") {
                        name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string()
                    } else {
                        child
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string()
                    };
                    imports.push(ParsedImport {
                        imported_name: name,
                        module_path: module_path.clone(),
                        line,
                        is_external: !is_relative,
                        original_name: None,
                    });
                }
                "wildcard_import" => {
                    imports.push(ParsedImport {
                        imported_name: "*".to_string(),
                        module_path: module_path.clone(),
                        line,
                        is_external: !is_relative,
                        original_name: None,
                    });
                }
                _ => {}
            }
        }

        // Fallback: if we found no individual names, emit the whole module as a single import.
        if imports.is_empty() {
            imports.push(ParsedImport {
                imported_name: module_path.clone(),
                module_path,
                line,
                is_external: !is_relative,
                original_name: None,
            });
        }

        imports
    }

    /// Walk an `import_statement` using AST children rather than text manipulation.
    /// For `import foo, bar.baz`, yields one ParsedImport per module.
    fn extract_plain_imports(node: tree_sitter::Node, source: &str) -> Vec<ParsedImport> {
        let line = node.start_position().row as u32 + 1;
        let mut imports = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "dotted_name" => {
                    let module = child
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    imports.push(ParsedImport {
                        imported_name: module.clone(),
                        module_path: module,
                        line,
                        is_external: true,
                        original_name: None,
                    });
                }
                "aliased_import" => {
                    let module = if let Some(name_node) = child.child_by_field_name("name") {
                        name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string()
                    } else {
                        child
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string()
                    };
                    imports.push(ParsedImport {
                        imported_name: module.clone(),
                        module_path: module,
                        line,
                        is_external: true,
                        original_name: None,
                    });
                }
                _ => {}
            }
        }
        imports
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
                "decorated_definition" => {
                    // A `decorated_definition` wraps decorators + a function/class definition.
                    let decorators = Self::extract_decorators(child, source);

                    // Find the inner definition (function_definition or class_definition).
                    let mut inner_cursor = child.walk();
                    for inner_child in child.children(&mut inner_cursor) {
                        match inner_child.kind() {
                            "function_definition" => {
                                if let Some(name_node) = inner_child.child_by_field_name("name") {
                                    let name = name_node
                                        .utf8_text(source.as_bytes())
                                        .unwrap_or_default()
                                        .to_string();
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
                                        line_start: inner_child.start_position().row as u32 + 1,
                                        line_end: inner_child.end_position().row as u32 + 1,
                                        // Methods (parent_name.is_some()) are not module-level
                                        // exports — they're only accessible through an instance.
                                        // Only top-level functions/classes are exported.
                                        is_exported: parent_name.is_none()
                                            && !name.starts_with('_'),
                                        signature: inner_child
                                            .utf8_text(source.as_bytes())
                                            .unwrap_or_default()
                                            .lines()
                                            .next()
                                            .unwrap_or_default()
                                            .to_string(),
                                        decorators: decorators.clone(),
                                        parent_name: parent_name.map(String::from),
                                    });
                                    Self::extract_symbols(
                                        inner_child,
                                        source,
                                        result,
                                        Some(&qualified),
                                    );
                                }
                            }
                            "class_definition" => {
                                if let Some(name_node) = inner_child.child_by_field_name("name") {
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
                                        qualified_name: qualified.clone(),
                                        kind: SymbolKind::Class,
                                        line_start: inner_child.start_position().row as u32 + 1,
                                        line_end: inner_child.end_position().row as u32 + 1,
                                        is_exported: parent_name.is_none()
                                            && !name.starts_with('_'),
                                        signature: format!("class {}", name),
                                        decorators: decorators.clone(),
                                        parent_name: parent_name.map(String::from),
                                    });
                                    Self::extract_symbols(
                                        inner_child,
                                        source,
                                        result,
                                        Some(&qualified),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "function_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
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
                            // Only top-level functions (no parent class) are module exports.
                            is_exported: parent_name.is_none() && !name.starts_with('_'),
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
                "class_definition" => {
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
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Class,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            // Only top-level classes are module exports.
                            is_exported: parent_name.is_none() && !name.starts_with('_'),
                            signature: format!("class {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                        Self::extract_symbols(child, source, result, Some(&qualified));
                    }
                }
                "import_statement" => {
                    let imports = Self::extract_plain_imports(child, source);
                    result.imports.extend(imports);
                }
                "import_from_statement" => {
                    let imports = Self::extract_from_imports(child, source);
                    result.imports.extend(imports);
                }
                "lambda" => {
                    let line = child.start_position().row as u32 + 1;
                    let name = format!("lambda_line_{}", line);
                    let qualified = match parent_name {
                        Some(p) => format!("{}.{}", p, name),
                        None => name.clone(),
                    };
                    result.symbols.push(ParsedSymbol {
                        name: name.clone(),
                        qualified_name: qualified,
                        kind: SymbolKind::Function,
                        line_start: line,
                        line_end: child.end_position().row as u32 + 1,
                        is_exported: false,
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
                    // Recurse into lambda body to find calls.
                    Self::extract_symbols(child, source, result, parent_name);
                }
                "call" => {
                    if let Some(func_node) = child.child_by_field_name("function") {
                        let callee = func_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
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
        let mut result = ParseResult {
            syntax_error_count: crate::parse::count_syntax_errors(root),
            ..ParseResult::default()
        };

        Self::extract_symbols(root, source, &mut result, None);

        Ok(result)
    }
}
