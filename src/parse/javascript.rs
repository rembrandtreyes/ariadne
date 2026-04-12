use crate::parse::types::{
    Language, ParseResult, ParsedCall, ParsedImport, ParsedSymbol, SymbolKind,
};
use crate::parse::LanguageParser;

pub struct JavaScriptParser;

impl Default for JavaScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaScriptParser {
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
                "lexical_declaration" | "variable_declaration" => {
                    Self::extract_variable_declarators(child, source, result, parent_name);
                }

                // ── Import statements ────────────────────────────────────
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

                // ── JSX elements as calls (tree-sitter-javascript supports JSX) ──
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

                // ── CommonJS: module.exports = { ... } ──────────────────
                "expression_statement" => {
                    Self::extract_commonjs_exports(child, source, result);
                    Self::extract_symbols(child, source, result, parent_name);
                }

                // ── Default: recurse ────────────────────────────────────
                _ => {
                    Self::extract_symbols(child, source, result, parent_name);
                }
            }
        }
    }

    /// Extract individual imports from an import_statement node.
    fn extract_imports(import_node: tree_sitter::Node, source: &str, result: &mut ParseResult) {
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

        let mut cursor = import_node.walk();
        for child in import_node.children(&mut cursor) {
            match child.kind() {
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

        // Side-effect import: `import 'module'`
        if !found_any {
            result.imports.push(ParsedImport {
                imported_name: module.clone(),
                module_path: module,
                line,
                is_external,
            });
        }
    }

    /// Walk an import_clause node and extract all specifiers.
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
                "namespace_import" => {
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
                "named_imports" => {
                    let mut named_cursor = child.walk();
                    for spec in child.children(&mut named_cursor) {
                        if spec.kind() == "import_specifier" {
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

                if is_function {
                    if let Some(v) = value_node {
                        Self::extract_symbols(v, source, result, Some(&qualified));
                    }
                } else {
                    Self::extract_symbols(child, source, result, parent_name);
                }
            }
        }
    }

    /// Extract JSX element usage as a call expression.
    fn extract_jsx_call(
        jsx_node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
        parent_name: Option<&str>,
    ) {
        let mut cursor = jsx_node.walk();
        for child in jsx_node.children(&mut cursor) {
            match child.kind() {
                "identifier" | "member_expression" | "nested_identifier" => {
                    let tag_name = child
                        .utf8_text(source.as_bytes())
                        .unwrap_or_default()
                        .to_string();
                    if tag_name.starts_with(|c: char| c.is_uppercase()) {
                        let caller = parent_name.unwrap_or("<module>").to_string();
                        result.calls.push(ParsedCall {
                            caller_name: caller,
                            callee_name: tag_name,
                            line: jsx_node.start_position().row as u32 + 1,
                        });
                    }
                    break;
                }
                _ => {}
            }
        }
        Self::extract_symbols(jsx_node, source, result, parent_name);
    }

    /// Detect CommonJS `module.exports = { name1, name2 }` and mark symbols as exported.
    fn extract_commonjs_exports(
        expr_node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
    ) {
        // Look for assignment_expression where left is module.exports
        let mut cursor = expr_node.walk();
        for child in expr_node.children(&mut cursor) {
            if child.kind() == "assignment_expression" {
                let left = match child.child_by_field_name("left") {
                    Some(l) => l,
                    None => continue,
                };
                let left_text = left.utf8_text(source.as_bytes()).unwrap_or_default();
                if left_text != "module.exports" {
                    continue;
                }
                let right = match child.child_by_field_name("right") {
                    Some(r) => r,
                    None => continue,
                };
                if right.kind() == "object" {
                    // module.exports = { name1, name2, key: value }
                    let mut obj_cursor = right.walk();
                    for prop in right.children(&mut obj_cursor) {
                        let export_name = match prop.kind() {
                            // Shorthand: module.exports = { getUsers }
                            "shorthand_property_identifier"
                            | "shorthand_property_identifier_pattern" => prop
                                .utf8_text(source.as_bytes())
                                .unwrap_or_default()
                                .to_string(),
                            // Key-value: module.exports = { getUsers: getUsers }
                            "pair" => {
                                if let Some(key) = prop.child_by_field_name("key") {
                                    key.utf8_text(source.as_bytes())
                                        .unwrap_or_default()
                                        .to_string()
                                } else {
                                    continue;
                                }
                            }
                            _ => continue,
                        };
                        for sym in result.symbols.iter_mut() {
                            if sym.name == export_name {
                                sym.is_exported = true;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle export statements.
    fn extract_export(
        export_node: tree_sitter::Node,
        source: &str,
        result: &mut ParseResult,
        parent_name: Option<&str>,
    ) {
        let symbols_before = result.symbols.len();
        let mut has_default = false;
        let mut has_export_clause = false;

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
            let mut clause_cursor = export_node.walk();
            for child in export_node.children(&mut clause_cursor) {
                if child.kind() == "export_clause" {
                    let mut spec_cursor = child.walk();
                    for spec in child.children(&mut spec_cursor) {
                        if spec.kind() == "export_specifier" {
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
            Self::extract_symbols(export_node, source, result, parent_name);

            for sym in result.symbols[symbols_before..].iter_mut() {
                sym.is_exported = true;
            }

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

impl LanguageParser for JavaScriptParser {
    fn language(&self) -> Language {
        Language::JavaScript
    }

    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_javascript::LANGUAGE.into())?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

        let root = tree.root_node();
        let mut result = ParseResult::default();

        // Synthetic symbol that anchors module-level call edges.
        // Calls made at file scope have caller_name = "<module>"; without a
        // matching symbol in the DB they would be silently dropped in
        // pipeline/parsing.rs. Marked is_entry_point in dead_code.rs so the
        // BFS reachability analysis keeps all callee edges alive.
        result.symbols.push(ParsedSymbol {
            name: "<module>".to_string(),
            qualified_name: "<module>".to_string(),
            kind: SymbolKind::Module,
            line_start: 0,
            line_end: 0,
            is_exported: false,
            signature: String::new(),
            decorators: Vec::new(),
            parent_name: None,
        });

        Self::extract_symbols(root, source, &mut result, None);

        Ok(result)
    }
}
