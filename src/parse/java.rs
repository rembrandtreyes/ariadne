use crate::parse::types::{
    Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind,
};
use crate::parse::LanguageParser;

pub struct JavaParser;

impl Default for JavaParser {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaParser {
    pub fn new() -> Self {
        Self
    }

    /// Check if a Java declaration node has a `modifiers` child containing "public".
    /// In tree-sitter Java, modifiers are grouped under a `modifiers` node.
    fn has_public_modifier(node: tree_sitter::Node, source: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for modifier in child.children(&mut mod_cursor) {
                    let text = modifier.utf8_text(source.as_bytes()).unwrap_or_default();
                    if text == "public" {
                        return true;
                    }
                }
            }
        }
        false
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
                "class_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        };
                        let is_public = Self::has_public_modifier(child, source);
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Class,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: is_public,
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
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        };
                        let is_public = Self::has_public_modifier(child, source);
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind: SymbolKind::Interface,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: is_public,
                            signature: format!("interface {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                "enum_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        };
                        let is_public = Self::has_public_modifier(child, source);
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind: SymbolKind::Enum,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: is_public,
                            signature: format!("enum {}", name),
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
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        };
                        let is_public = Self::has_public_modifier(child, source);
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Method,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: is_public,
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
                "constructor_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}.{}", p, name),
                            None => name.clone(),
                        };
                        let is_public = Self::has_public_modifier(child, source);
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified.clone(),
                            kind: SymbolKind::Method,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: is_public,
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
                "import_declaration" => {
                    let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
                    let module = text
                        .trim_start_matches("import ")
                        .trim_end_matches(';')
                        .trim()
                        .to_string();
                    result.imports.push(ParsedImport {
                        imported_name: module
                            .split('.')
                            .next_back()
                            .unwrap_or_default()
                            .to_string(),
                        module_path: module,
                        line: child.start_position().row as u32 + 1,
                        is_external: true,
                        original_name: None,
                    });
                }
                "method_invocation" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let callee = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let caller = parent_name.unwrap_or("<class>").to_string();
                        result.calls.push(ParsedCall {
                            caller_name: caller,
                            callee_name: callee,
                            line: child.start_position().row as u32 + 1,
                        });
                    }
                    Self::extract_symbols(child, source, result, parent_name);
                }
                "object_creation_expression" => {
                    // Java `new Foo(...)` - extract the type being instantiated as a call
                    if let Some(type_node) = child.child_by_field_name("type") {
                        let callee = type_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let caller = parent_name.unwrap_or("<class>").to_string();
                        result.calls.push(ParsedCall {
                            caller_name: caller,
                            callee_name: callee,
                            line: child.start_position().row as u32 + 1,
                        });
                    }
                    Self::extract_symbols(child, source, result, parent_name);
                }
                _ => {
                    Self::extract_symbols(child, source, result, parent_name);
                }
            }
        }
    }
}

impl LanguageParser for JavaParser {
    fn language(&self) -> Language {
        Language::Java
    }

    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_java::LANGUAGE.into())?;

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
