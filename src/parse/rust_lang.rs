use crate::parse::types::{
    Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind,
};
use crate::parse::LanguageParser;

pub struct RustParser;

impl Default for RustParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RustParser {
    pub fn new() -> Self {
        Self
    }

    /// Returns true if any of the pending attribute texts indicate a test attribute.
    /// Matches `#[test]` and `#[cfg(test)]`.
    fn is_test_attribute(attrs: &[String]) -> bool {
        attrs.iter().any(|a| {
            let trimmed = a.trim();
            trimmed == "#[test]"
                || trimmed.starts_with("#[cfg(test)")
                || trimmed.starts_with("#[cfg(test,")
        })
    }

    /// Returns true if any of the pending attribute texts indicate cfg(test) on a module.
    fn is_cfg_test_attribute(attrs: &[String]) -> bool {
        attrs.iter().any(|a| {
            let t = a.trim();
            t.starts_with("#[cfg(test") || t.contains("cfg(test)")
        })
    }

    fn extract_symbols(
        node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
        parent_name: Option<&str>,
        in_test_module: bool,
        in_trait_impl: bool,
    ) {
        let mut cursor = node.walk();
        // Accumulate attribute_item nodes that precede the next declaration.
        // In tree-sitter-rust, #[test] and similar outer attributes appear as
        // sibling nodes immediately before the item they annotate.
        let mut pending_attrs: Vec<String> = Vec::new();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "attribute_item" => {
                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                        pending_attrs.push(text.trim().to_string());
                    }
                }
                "function_item" => {
                    let attrs = std::mem::take(&mut pending_attrs);
                    let is_test =
                        in_test_module || Self::is_test_attribute(&attrs);

                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        let qualified = match parent_name {
                            Some(p) => format!("{}::{}", p, name),
                            None => name.clone(),
                        };
                        let is_pub = Self::has_visibility(child, source);
                        let kind = if parent_name.is_some() {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };
                        let mut decorators = Vec::new();
                        if is_test {
                            decorators.push("test".to_string());
                        }
                        if in_trait_impl {
                            decorators.push("trait_impl".to_string());
                        }
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: is_pub,
                            signature: child
                                .utf8_text(source.as_bytes())
                                .unwrap_or_default()
                                .lines()
                                .next()
                                .unwrap_or_default()
                                .to_string(),
                            decorators,
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                "mod_item" => {
                    let attrs = std::mem::take(&mut pending_attrs);
                    // Propagate test context into cfg(test) modules so all
                    // functions inside are marked as test symbols.
                    let child_in_test = in_test_module || Self::is_cfg_test_attribute(&attrs);
                    let mod_name = child
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or_default()
                        .to_string();
                    let qualified = match parent_name {
                        Some(p) => format!("{}::{}", p, mod_name),
                        None => mod_name.clone(),
                    };
                    Self::extract_symbols(
                        child,
                        source,
                        result,
                        Some(&qualified),
                        child_in_test,
                        false, // new module resets trait_impl context
                    );
                }
                "struct_item" => {
                    pending_attrs.clear();
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
                            kind: SymbolKind::Class,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: Self::has_visibility(child, source),
                            signature: format!("struct {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                "enum_item" => {
                    pending_attrs.clear();
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
                            kind: SymbolKind::Enum,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: Self::has_visibility(child, source),
                            signature: format!("enum {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                "trait_item" => {
                    pending_attrs.clear();
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
                            kind: SymbolKind::Trait,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: Self::has_visibility(child, source),
                            signature: format!("trait {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                        Self::extract_symbols(child, source, result, Some(&qualified), in_test_module, false);
                    }
                }
                "impl_item" => {
                    pending_attrs.clear();
                    let type_name = child
                        .child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .unwrap_or_default()
                        .to_string();
                    // Detect `impl Trait for Type` — the "trait" field is only
                    // present for trait implementations, not plain `impl Type {}`.
                    let is_trait_impl = child.child_by_field_name("trait").is_some();
                    if !type_name.is_empty() {
                        Self::extract_symbols(child, source, result, Some(&type_name), in_test_module, is_trait_impl);
                    } else {
                        Self::extract_symbols(child, source, result, parent_name, in_test_module, in_trait_impl);
                    }
                }
                "use_declaration" => {
                    pending_attrs.clear();
                    let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
                    let module = text
                        .trim_start_matches("use ")
                        .trim_end_matches(';')
                        .trim()
                        .to_string();
                    let is_external = !module.starts_with("crate::")
                        && !module.starts_with("super::")
                        && !module.starts_with("self::");
                    result.imports.push(ParsedImport {
                        imported_name: module.split("::").last().unwrap_or_default().to_string(),
                        module_path: module,
                        line: child.start_position().row as u32 + 1,
                        is_external,
                    });
                }
                "call_expression" => {
                    pending_attrs.clear();
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
                "const_item" => {
                    pending_attrs.clear();
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
                            kind: SymbolKind::Constant,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: Self::has_visibility(child, source),
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
                "static_item" => {
                    pending_attrs.clear();
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
                            kind: SymbolKind::Constant,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: Self::has_visibility(child, source),
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
                "macro_invocation" => {
                    pending_attrs.clear();
                    // Extract macro name from the first child (the macro identifier before `!`)
                    let macro_node = child.child(0);
                    if let Some(macro_name_node) = macro_node {
                        let macro_name = macro_name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        if !macro_name.is_empty() {
                            let caller = parent_name.unwrap_or("<module>").to_string();
                            result.calls.push(ParsedCall {
                                caller_name: caller,
                                callee_name: format!("{}!", macro_name),
                                line: child.start_position().row as u32 + 1,
                            });
                        }
                    }
                    // Still recurse into macro invocations for nested content
                    Self::extract_symbols(child, source, result, parent_name, in_test_module, in_trait_impl);
                }
                "type_item" => {
                    pending_attrs.clear();
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
                            kind: SymbolKind::TypeAlias,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: Self::has_visibility(child, source),
                            signature: format!("type {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }
                _ => {
                    pending_attrs.clear();
                    Self::extract_symbols(child, source, result, parent_name, in_test_module, in_trait_impl);
                }
            }
        }
    }

    fn has_visibility(node: tree_sitter::Node, source: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
                if text.contains("pub") {
                    return true;
                }
            }
        }
        false
    }
}

impl LanguageParser for RustParser {
    fn language(&self) -> Language {
        Language::Rust
    }

    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

        let root = tree.root_node();
        let mut result = ParseResult::default();

        Self::extract_symbols(root, source, &mut result, None, false, false);

        Ok(result)
    }
}
