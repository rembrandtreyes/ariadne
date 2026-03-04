//! Ariadne Kotlin Plugin — Example WASM Language Plugin
//!
//! This plugin demonstrates how to implement Ariadne's WIT contract for a new
//! language. It performs basic line-by-line parsing of Kotlin source files to
//! extract symbols, imports, and call relationships.
//!
//! This is intentionally simple — a production plugin would use tree-sitter or
//! a proper parser for accurate AST-level extraction. The goal here is to show
//! the WIT bindgen pattern clearly.

// ---------------------------------------------------------------------------
// Step 1: Generate Rust bindings from the WIT contract.
//
// The `wit_bindgen::generate!` macro reads the WIT file and produces:
//   - Type definitions (LanguageInfo, ParsedSymbol, ParseResult, etc.)
//   - A `Guest` trait that you must implement
//   - An `export!` macro to wire your struct to the WASM exports
// ---------------------------------------------------------------------------
wit_bindgen::generate!({
    world: "ariadne-plugin",
    path: "wit",
});

// ---------------------------------------------------------------------------
// Step 2: Define your plugin struct. This is the type that implements `Guest`.
// ---------------------------------------------------------------------------
struct KotlinPlugin;

// ---------------------------------------------------------------------------
// Step 3: Register the struct as the exported implementation.
//
// The `export!` macro generates the WASM export glue that the Ariadne host
// calls into. Without this line, the component has no exports.
// ---------------------------------------------------------------------------
export!(KotlinPlugin);

// ---------------------------------------------------------------------------
// Step 4: Implement the Guest trait — this is the core of your plugin.
// ---------------------------------------------------------------------------
impl Guest for KotlinPlugin {
    /// Returns metadata about this plugin.
    ///
    /// The host calls `get-info()` once at startup to learn:
    /// - Which language this plugin handles
    /// - Which file extensions to route to this plugin
    /// - The plugin version (for diagnostics)
    fn get_info() -> LanguageInfo {
        LanguageInfo {
            name: "kotlin".to_string(),
            extensions: vec![".kt".to_string(), ".kts".to_string()],
            version: "0.1.0".to_string(),
        }
    }

    /// Parses a Kotlin source file and returns structured data.
    ///
    /// The host passes:
    /// - `source`: the full file contents as a string
    /// - `file_path`: the path to the file (for context, not filesystem access)
    ///
    /// This example uses simple line-by-line string matching. A production
    /// plugin would use a proper parser for accuracy.
    fn parse_file(source: String, _file_path: String) -> ParseResult {
        let mut symbols: Vec<ParsedSymbol> = Vec::new();
        let mut imports: Vec<ParsedImport> = Vec::new();
        let mut calls: Vec<ParsedCall> = Vec::new();

        // Track the current class/object scope for parent-name and caller context
        let mut current_class: Option<String> = None;
        let mut current_function: Option<String> = None;

        for (line_idx, line) in source.lines().enumerate() {
            let line_num = (line_idx + 1) as u32;
            let trimmed = line.trim();

            // -----------------------------------------------------------------
            // Detect imports: `import com.example.Foo`
            // -----------------------------------------------------------------
            if trimmed.starts_with("import ") {
                let path = trimmed
                    .strip_prefix("import ")
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches(';');

                // The imported name is the last segment of the path
                let imported_name = path
                    .rsplit('.')
                    .next()
                    .unwrap_or(path)
                    .to_string();

                // Heuristic: external if it doesn't start with the project's
                // package. Since we don't know the project package, we treat
                // everything as potentially external.
                imports.push(ParsedImport {
                    imported_name,
                    module_path: path.to_string(),
                    line: line_num,
                    is_external: true,
                });
                continue;
            }

            // -----------------------------------------------------------------
            // Detect class declarations: `class Foo`, `data class Foo`,
            // `abstract class Foo`, `open class Foo`, `object Foo`
            // -----------------------------------------------------------------
            if let Some(class_name) = extract_class_name(trimmed) {
                let is_exported = !trimmed.contains("private ")
                    && !trimmed.contains("internal ");

                let kind = if trimmed.contains("interface ") {
                    SymbolKind::Interface
                } else if trimmed.contains("enum ") {
                    SymbolKind::EnumType
                } else if trimmed.contains("object ") {
                    SymbolKind::Module
                } else {
                    SymbolKind::Class
                };

                symbols.push(ParsedSymbol {
                    name: class_name.clone(),
                    qualified_name: class_name.clone(),
                    kind,
                    line_start: line_num,
                    line_end: line_num, // Simplified — real parser would find the closing brace
                    is_exported,
                    signature: trimmed.to_string(),
                    decorators: Vec::new(),
                    parent_name: None,
                });

                current_class = Some(class_name);
                continue;
            }

            // -----------------------------------------------------------------
            // Detect function declarations: `fun doSomething(`
            // Also handles: `private fun`, `override fun`, `suspend fun`, etc.
            // -----------------------------------------------------------------
            if let Some(func_name) = extract_function_name(trimmed) {
                let is_exported = !trimmed.contains("private ")
                    && !trimmed.contains("internal ");

                let kind = if current_class.is_some() {
                    SymbolKind::Method
                } else {
                    SymbolKind::Function
                };

                let qualified = match &current_class {
                    Some(cls) => format!("{}.{}", cls, func_name),
                    None => func_name.clone(),
                };

                symbols.push(ParsedSymbol {
                    name: func_name.clone(),
                    qualified_name: qualified,
                    kind,
                    line_start: line_num,
                    line_end: line_num,
                    is_exported,
                    signature: trimmed.to_string(),
                    decorators: Vec::new(),
                    parent_name: current_class.clone(),
                });

                current_function = Some(func_name);
                continue;
            }

            // -----------------------------------------------------------------
            // Detect function calls: `someName(` patterns within code lines.
            // Skip keywords that look like calls but aren't (if, for, etc.)
            // -----------------------------------------------------------------
            if let Some(caller) = current_function.as_ref().or(current_class.as_ref()) {
                for callee in extract_calls(trimmed) {
                    calls.push(ParsedCall {
                        caller_name: caller.clone(),
                        callee_name: callee,
                        line: line_num,
                    });
                }
            }

            // -----------------------------------------------------------------
            // Detect scope exit (simplified: closing brace at start of line)
            // -----------------------------------------------------------------
            if trimmed == "}" {
                if current_function.is_some() {
                    current_function = None;
                } else {
                    current_class = None;
                }
            }
        }

        // Return the complete parse result. Empty vectors for fields we don't
        // populate (api_endpoints, api_calls) — this is totally fine and has
        // near-zero overhead.
        ParseResult {
            symbols,
            imports,
            calls,
            api_endpoints: Vec::new(),
            api_calls: Vec::new(),
        }
    }
}

// ===========================================================================
// Helper functions — simple string extraction utilities
// ===========================================================================

/// Extracts a class/interface/object/enum name from a declaration line.
///
/// Handles patterns like:
///   `class Foo {`
///   `data class Foo(`
///   `abstract class Foo : Bar {`
///   `interface Foo {`
///   `object Foo {`
///   `enum class Color {`
fn extract_class_name(line: &str) -> Option<String> {
    // Look for "class ", "interface ", or "object " keyword
    let keywords = ["class ", "interface ", "object "];

    for keyword in &keywords {
        if let Some(idx) = line.find(keyword) {
            let after = &line[idx + keyword.len()..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();

            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    None
}

/// Extracts a function name from a `fun` declaration line.
///
/// Handles:
///   `fun main() {`
///   `private fun helper(x: Int): String {`
///   `override suspend fun fetchData(): Result<Data> {`
fn extract_function_name(line: &str) -> Option<String> {
    let fun_idx = line.find("fun ")?;
    let after_fun = &line[fun_idx + 4..];

    // Skip generic type parameters like `<T>`
    let after_generics = if after_fun.starts_with('<') {
        let end = after_fun.find('>')? + 1;
        &after_fun[end..]
    } else {
        after_fun
    };

    let name: String = after_generics
        .trim()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Extracts function call names from a line of code.
///
/// Looks for `identifier(` patterns while filtering out Kotlin keywords that
/// use parentheses (if, for, while, when, return, etc.)
fn extract_calls(line: &str) -> Vec<String> {
    let keywords = [
        "if", "for", "while", "when", "return", "throw", "catch", "class",
        "fun", "val", "var", "import", "package", "else", "try",
    ];

    let mut results = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();

    let mut i = 0;
    while i < len {
        // Skip string literals
        if chars[i] == '"' {
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1; // skip escaped char
                }
                i += 1;
            }
            i += 1;
            continue;
        }

        // Look for `name(` pattern
        if chars[i] == '(' && i > 0 {
            // Walk backwards to find the identifier
            let end = i;
            let mut start = i;
            while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                start -= 1;
            }

            if start < end {
                let name: String = chars[start..end].iter().collect();
                if !name.is_empty() && !keywords.contains(&name.as_str()) {
                    results.push(name);
                }
            }
        }

        i += 1;
    }

    results
}
