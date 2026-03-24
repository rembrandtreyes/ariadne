use crate::parse::types::{
    Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind,
};
use crate::parse::LanguageParser;

pub struct TypeScriptParser;

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeScriptParser {
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
                // ── Function declarations ────────────────────────────────
                "function_declaration" | "generator_function_declaration" => {
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
                            kind: SymbolKind::Function,
                            line_start: child.start_position().row as u32 + 1,
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
                        Self::extract_symbols(child, source, result, Some(&qualified));
                    }
                }

                // ── Class declarations ───────────────────────────────────
                "class_declaration" | "abstract_class_declaration" => {
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
                            is_exported: false,
                            signature: format!("class {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                        Self::extract_symbols(child, source, result, Some(&qualified));
                    }
                }

                // ── Interface declarations (TS-specific) ─────────────────
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
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind: SymbolKind::Interface,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: false,
                            signature: format!("interface {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }

                // ── Type alias declarations (TS-specific) ────────────────
                "type_alias_declaration" => {
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
                            kind: SymbolKind::TypeAlias,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: false,
                            signature: format!("type {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }

                // ── Enum declarations (TS-specific) ──────────────────────
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
                        result.symbols.push(ParsedSymbol {
                            name: name.clone(),
                            qualified_name: qualified,
                            kind: SymbolKind::Enum,
                            line_start: child.start_position().row as u32 + 1,
                            line_end: child.end_position().row as u32 + 1,
                            is_exported: false,
                            signature: format!("enum {}", name),
                            decorators: Vec::new(),
                            parent_name: parent_name.map(String::from),
                        });
                    }
                }

                // ── Method definitions (inside classes) ──────────────────
                "method_definition" => {
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
                    }
                }

                // ── Lexical declarations (const/let) & variable declarations (var) ──
                // Extract arrow functions and function expressions as Function symbols.
                // Extract other const bindings as Constant symbols.
                "lexical_declaration" | "variable_declaration" => {
                    Self::extract_variable_declarators(child, source, result, parent_name);
                }

                // ── Import statements ────────────────────────────────────
                // Properly extract each named import, default import, and namespace import.
                "import_statement" => {
                    Self::extract_imports(child, source, result);
                }

                // ── Call expressions ─────────────────────────────────────
                "call_expression" | "new_expression" => {
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
                    // Recurse into arguments to find nested calls
                    Self::extract_symbols(child, source, result, parent_name);
                }

                // ── JSX elements as calls ────────────────────────────────
                "jsx_self_closing_element" => {
                    Self::extract_jsx_call(child, source, result, parent_name);
                }
                "jsx_opening_element" => {
                    Self::extract_jsx_call(child, source, result, parent_name);
                }

                // ── Export statements ─────────────────────────────────────
                "export_statement" => {
                    Self::extract_export(child, source, result, parent_name);
                }

                // ── Default: recurse ────────────────────────────────────
                _ => {
                    Self::extract_symbols(child, source, result, parent_name);
                }
            }
        }
    }

    /// Extract individual imports from an import_statement node.
    /// Handles: named imports, default imports, namespace imports, and side-effect imports.
    fn extract_imports(import_node: tree_sitter::Node, source: &str, result: &mut ParseResult) {
        // Get the module path from the `source` field of the import_statement
        let module = match import_node.child_by_field_name("source") {
            Some(source_node) => {
                let raw = source_node.utf8_text(source.as_bytes()).unwrap_or_default();
                raw.trim_matches('\'').trim_matches('"').to_string()
            }
            None => return,
        };
        let is_external = !module.starts_with('.') && !module.starts_with('/');
        let line = import_node.start_position().row as u32 + 1;

        let mut found_any = false;

        // Walk direct children to find import clauses
        let mut cursor = import_node.walk();
        for child in import_node.children(&mut cursor) {
            match child.kind() {
                // `import X from 'mod'` - default import (identifier as direct child of import_clause or import_statement)
                "identifier" => {
                    let name = child
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    if !name.is_empty() {
                        result.imports.push(ParsedImport {
                            imported_name: name,
                            module_path: module.clone(),
                            line,
                            is_external,
                        });
                        found_any = true;
                    }
                }
                // import_clause contains the actual specifiers
                "import_clause" => {
                    Self::extract_import_clause(
                        child,
                        source,
                        &module,
                        line,
                        is_external,
                        result,
                        &mut found_any,
                    );
                }
                _ => {}
            }
        }

        // Side-effect import: `import 'module'` - no specifiers
        if !found_any {
            result.imports.push(ParsedImport {
                imported_name: module.clone(),
                module_path: module,
                line,
                is_external,
            });
        }
    }

    /// Walk an import_clause node and extract all specifiers from it.
    fn extract_import_clause(
        clause_node: tree_sitter::Node,
        source: &str,
        module: &str,
        line: u32,
        is_external: bool,
        result: &mut ParseResult,
        found_any: &mut bool,
    ) {
        let mut cursor = clause_node.walk();
        for child in clause_node.children(&mut cursor) {
            match child.kind() {
                // Default import: `import React from 'react'`
                "identifier" => {
                    let name = child
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    if !name.is_empty() {
                        result.imports.push(ParsedImport {
                            imported_name: name,
                            module_path: module.to_string(),
                            line,
                            is_external,
                        });
                        *found_any = true;
                    }
                }
                // Namespace import: `import * as X from 'bar'`
                "namespace_import" => {
                    // The identifier after `as` is the name we want
                    let mut ns_cursor = child.walk();
                    for ns_child in child.children(&mut ns_cursor) {
                        if ns_child.kind() == "identifier" {
                            let name = ns_child
                                .utf8_text(source.as_bytes())
                                .unwrap_or_default()
                                .to_string();
                            if !name.is_empty() {
                                result.imports.push(ParsedImport {
                                    imported_name: name,
                                    module_path: module.to_string(),
                                    line,
                                    is_external,
                                });
                                *found_any = true;
                            }
                        }
                    }
                }
                // Named imports: `import { a, b as c } from 'mod'`
                "named_imports" => {
                    let mut named_cursor = child.walk();
                    for spec in child.children(&mut named_cursor) {
                        if spec.kind() == "import_specifier" {
                            // If there is an `alias` field, that is the local name.
                            // The `name` field is the original exported name.
                            let imported_name =
                                if let Some(alias_node) = spec.child_by_field_name("alias") {
                                    alias_node
                                        .utf8_text(source.as_bytes())
                                        .unwrap_or_default()
                                        .to_string()
                                } else if let Some(name_node) = spec.child_by_field_name("name") {
                                    name_node
                                        .utf8_text(source.as_bytes())
                                        .unwrap_or_default()
                                        .to_string()
                                } else {
                                    // Fallback: use the full text of the specifier
                                    spec.utf8_text(source.as_bytes())
                                        .unwrap_or_default()
                                        .trim()
                                        .to_string()
                                };
                            if !imported_name.is_empty() {
                                result.imports.push(ParsedImport {
                                    imported_name,
                                    module_path: module.to_string(),
                                    line,
                                    is_external,
                                });
                                *found_any = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Extract variable declarators from const/let/var declarations.
    /// Arrow functions and function expressions become Function symbols.
    /// Other bindings become Constant (for const) or Variable (for let/var).
    fn extract_variable_declarators(
        decl_node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
        parent_name: Option<&str>,
    ) {
        let is_const = decl_node.kind() == "lexical_declaration" && {
            let mut c = decl_node.walk();
            let result = decl_node.children(&mut c).any(|ch| {
                ch.kind() == "const"
                    || ch.utf8_text(source.as_bytes()).unwrap_or_default() == "const"
            });
            result
        };

        let mut cursor = decl_node.walk();
        for child in decl_node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                let name = match child.child_by_field_name("name") {
                    Some(n) => {
                        let text = n
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        // Skip destructuring patterns (they start with { or [)
                        if text.starts_with('{') || text.starts_with('[') {
                            continue;
                        }
                        text
                    }
                    None => continue,
                };

                let qualified = match parent_name {
                    Some(p) => format!("{}.{}", p, name),
                    None => name.clone(),
                };

                // Check if the value is an arrow function or function expression
                let value_node = child.child_by_field_name("value");
                let is_function = value_node.is_some_and(|v| {
                    matches!(
                        v.kind(),
                        "arrow_function" | "function_expression" | "function"
                    )
                });

                let kind = if is_function {
                    SymbolKind::Function
                } else if is_const {
                    SymbolKind::Constant
                } else {
                    SymbolKind::Variable
                };

                let sig = decl_node
                    .utf8_text(source.as_bytes())
                    .unwrap_or_default()
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string();

                result.symbols.push(ParsedSymbol {
                    name: name.clone(),
                    qualified_name: qualified.clone(),
                    kind,
                    line_start: child.start_position().row as u32 + 1,
                    line_end: child.end_position().row as u32 + 1,
                    is_exported: false,
                    signature: sig,
                    decorators: Vec::new(),
                    parent_name: parent_name.map(String::from),
                });

                // Recurse into the function body if it is a function value
                if is_function {
                    if let Some(v) = value_node {
                        Self::extract_symbols(v, source, result, Some(&qualified));
                    }
                } else {
                    // Still recurse to find nested calls
                    Self::extract_symbols(child, source, result, parent_name);
                }
            }
        }
    }

    /// Extract JSX element usage as a call expression.
    /// Only components (uppercase first letter) are treated as calls.
    fn extract_jsx_call(
        jsx_node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
        parent_name: Option<&str>,
    ) {
        // The tag name is typically the first child that is an identifier or member_expression
        let mut cursor = jsx_node.walk();
        for child in jsx_node.children(&mut cursor) {
            match child.kind() {
                "identifier" | "member_expression" | "nested_identifier" => {
                    let tag_name = child
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    // Only treat uppercase-starting tags as component calls
                    // (lowercase tags like <div> are HTML elements)
                    if tag_name.starts_with(|c: char| c.is_uppercase()) {
                        let caller = parent_name.unwrap_or("<module>").to_string();
                        result.calls.push(ParsedCall {
                            caller_name: caller,
                            callee_name: tag_name,
                            line: jsx_node.start_position().row as u32 + 1,
                        });
                    }
                    break; // Only process the first identifier (the tag name)
                }
                _ => {}
            }
        }
        // Recurse into children to find nested JSX calls and regular calls
        Self::extract_symbols(jsx_node, source, result, parent_name);
    }

    /// Handle export statements.
    /// - `export function/class/const ...` -- recurse and mark symbols
    /// - `export default ...` -- recurse, mark as exported
    /// - `export { a, b }` -- look up existing symbols and mark as exported
    fn extract_export(
        export_node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
        parent_name: Option<&str>,
    ) {
        let symbols_before = result.symbols.len();
        let mut has_default = false;
        let mut has_export_clause = false;

        // Check for `default` keyword and `export_clause`
        let mut cursor = export_node.walk();
        for child in export_node.children(&mut cursor) {
            if child.kind() == "default"
                || (child.kind() == "identifier"
                    && child.utf8_text(source.as_bytes()).unwrap_or_default() == "default")
            {
                has_default = true;
            }
            if child.kind() == "export_clause" {
                has_export_clause = true;
            }
        }

        if has_export_clause {
            // Handle `export { name1, name2 }` - look up existing symbols by name
            let mut clause_cursor = export_node.walk();
            for child in export_node.children(&mut clause_cursor) {
                if child.kind() == "export_clause" {
                    let mut spec_cursor = child.walk();
                    for spec in child.children(&mut spec_cursor) {
                        if spec.kind() == "export_specifier" {
                            // The `name` field is the local name being exported
                            let exported_name =
                                if let Some(name_node) = spec.child_by_field_name("name") {
                                    name_node
                                        .utf8_text(source.as_bytes())
                                        .unwrap_or_default()
                                        .to_string()
                                } else {
                                    spec.utf8_text(source.as_bytes())
                                        .unwrap_or_default()
                                        .trim()
                                        .to_string()
                                };
                            // Mark matching symbols as exported
                            for sym in result.symbols.iter_mut() {
                                if sym.name == exported_name {
                                    sym.is_exported = true;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Recurse into the export statement to extract child declarations
            Self::extract_symbols(export_node, source, result, parent_name);

            // Mark all symbols that were added during this export as exported
            for sym in result.symbols[symbols_before..].iter_mut() {
                sym.is_exported = true;
            }

            // For `export default expression` where expression is an identifier (not a declaration),
            // try to look up the existing symbol and mark it as exported
            if has_default && result.symbols.len() == symbols_before {
                let mut id_cursor = export_node.walk();
                for child in export_node.children(&mut id_cursor) {
                    if child.kind() == "identifier" {
                        let name = child
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_string();
                        if name != "default" {
                            for sym in result.symbols.iter_mut() {
                                if sym.name == name {
                                    sym.is_exported = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl LanguageParser for TypeScriptParser {
    fn language(&self) -> Language {
        Language::TypeScript
    }

    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

        let root = tree.root_node();
        let mut result = ParseResult::default();

        Self::extract_symbols(root, source, &mut result, None);

        Ok(result)
    }
}
