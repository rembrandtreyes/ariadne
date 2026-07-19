use ariadne::parse::get_parser;
use ariadne::parse::types::{Language, SymbolKind};

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

#[test]
fn test_python_parser() {
    let parser = get_parser(Language::Python);
    let source =
        std::fs::read_to_string("tests/fixtures/python_repo/main.py").expect("fixture exists");
    let result = parser
        .parse_file(&source, "main.py")
        .expect("parse succeeds");
    assert!(!result.symbols.is_empty(), "should find symbols");
    assert!(!result.calls.is_empty(), "should find calls");
}

// ---------------------------------------------------------------------------
// JavaScript
// ---------------------------------------------------------------------------

#[test]
fn test_javascript_parser() {
    let parser = get_parser(Language::JavaScript);
    let source =
        std::fs::read_to_string("tests/fixtures/js_ts_repo/index.js").expect("fixture exists");
    let result = parser
        .parse_file(&source, "index.js")
        .expect("parse succeeds");
    assert!(!result.symbols.is_empty(), "should find symbols");
}

// ---------------------------------------------------------------------------
// TypeScript
// ---------------------------------------------------------------------------

#[test]
fn test_typescript_parser() {
    let parser = get_parser(Language::TypeScript);
    let source =
        std::fs::read_to_string("tests/fixtures/js_ts_repo/utils.ts").expect("fixture exists");
    let result = parser
        .parse_file(&source, "utils.ts")
        .expect("parse succeeds");
    assert!(!result.symbols.is_empty(), "should find symbols");
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

#[test]
fn test_go_parser() {
    let parser = get_parser(Language::Go);
    let source = std::fs::read_to_string("tests/fixtures/go_repo/main.go").expect("fixture exists");
    let result = parser
        .parse_file(&source, "main.go")
        .expect("parse succeeds");
    assert!(!result.symbols.is_empty(), "should find symbols");
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

#[test]
fn test_java_parser() {
    let parser = get_parser(Language::Java);
    let source =
        std::fs::read_to_string("tests/fixtures/java_repo/Main.java").expect("fixture exists");
    let result = parser
        .parse_file(&source, "Main.java")
        .expect("parse succeeds");
    assert!(!result.symbols.is_empty(), "should find symbols");
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

#[test]
fn test_rust_parser() {
    let parser = get_parser(Language::Rust);
    let source =
        std::fs::read_to_string("tests/fixtures/rust_repo/lib.rs").expect("fixture exists");
    let result = parser
        .parse_file(&source, "lib.rs")
        .expect("parse succeeds");
    assert!(!result.symbols.is_empty(), "should find symbols");
    // Rust structs are mapped to SymbolKind::Class
    let has_struct = result.symbols.iter().any(|s| s.kind == SymbolKind::Class);
    assert!(has_struct, "should find structs (mapped to Class)");
    // Function-to-function calls should be detected
    assert!(
        !result.calls.is_empty(),
        "should find calls inside function bodies"
    );
    // welcome() calls greet()
    let calls_greet = result
        .calls
        .iter()
        .any(|c| c.callee_name == "greet" && c.caller_name == "welcome");
    assert!(calls_greet, "welcome should call greet");
    // greet_user() calls get_user via self
    let calls_get_user = result
        .calls
        .iter()
        .any(|c| c.callee_name == "get_user" && c.caller_name == "greet_user");
    assert!(calls_get_user, "greet_user should call get_user");
}

// ---------------------------------------------------------------------------
// C#
// ---------------------------------------------------------------------------

#[test]
fn test_csharp_parser() {
    let parser = get_parser(Language::CSharp);
    let source =
        std::fs::read_to_string("tests/fixtures/csharp_repo/Program.cs").expect("fixture exists");
    let result = parser
        .parse_file(&source, "Program.cs")
        .expect("parse succeeds");

    assert!(!result.symbols.is_empty(), "should find symbols");

    let has_class = result.symbols.iter().any(|s| s.kind == SymbolKind::Class);
    assert!(has_class, "should find classes");

    let has_interface = result
        .symbols
        .iter()
        .any(|s| s.kind == SymbolKind::Interface);
    assert!(has_interface, "should find interfaces");

    let has_method = result.symbols.iter().any(|s| s.kind == SymbolKind::Method);
    assert!(has_method, "should find methods");

    assert!(!result.imports.is_empty(), "should find using directives");
}

// ---------------------------------------------------------------------------
// Ruby
// ---------------------------------------------------------------------------

#[test]
fn test_ruby_parser() {
    let parser = get_parser(Language::Ruby);
    let source =
        std::fs::read_to_string("tests/fixtures/ruby_repo/app.rb").expect("fixture exists");
    let result = parser
        .parse_file(&source, "app.rb")
        .expect("parse succeeds");

    assert!(!result.symbols.is_empty(), "should find symbols");

    let has_class = result.symbols.iter().any(|s| s.kind == SymbolKind::Class);
    assert!(has_class, "should find classes");

    let has_module = result.symbols.iter().any(|s| s.kind == SymbolKind::Module);
    assert!(has_module, "should find modules");

    let has_method = result
        .symbols
        .iter()
        .any(|s| s.kind == SymbolKind::Method || s.kind == SymbolKind::Function);
    assert!(has_method, "should find methods");

    assert!(!result.imports.is_empty(), "should find require statements");
}

// ---------------------------------------------------------------------------
// PHP
// ---------------------------------------------------------------------------

#[test]
fn test_php_parser() {
    let parser = get_parser(Language::Php);
    let source =
        std::fs::read_to_string("tests/fixtures/php_repo/index.php").expect("fixture exists");
    let result = parser
        .parse_file(&source, "index.php")
        .expect("parse succeeds");

    assert!(!result.symbols.is_empty(), "should find symbols");

    let has_class = result.symbols.iter().any(|s| s.kind == SymbolKind::Class);
    assert!(has_class, "should find classes");

    let has_interface = result
        .symbols
        .iter()
        .any(|s| s.kind == SymbolKind::Interface);
    assert!(has_interface, "should find interfaces");

    assert!(!result.imports.is_empty(), "should find use statements");
}

// ---------------------------------------------------------------------------
// Cross-language: all parsers exist
// ---------------------------------------------------------------------------

#[test]
fn test_all_parsers_exist() {
    let languages = [
        Language::Python,
        Language::JavaScript,
        Language::TypeScript,
        Language::Go,
        Language::Java,
        Language::Rust,
        Language::CSharp,
        Language::Ruby,
        Language::Php,
    ];
    for lang in &languages {
        let parser = get_parser(*lang);
        assert_eq!(parser.language(), *lang);
    }
}

// ---------------------------------------------------------------------------
// Cross-language: minimal source does not panic
// ---------------------------------------------------------------------------

#[test]
fn test_all_languages_parse_without_panic() {
    let test_cases = [
        (Language::Python, "def foo(): pass"),
        (Language::JavaScript, "function foo() {}"),
        (Language::TypeScript, "function foo(): void {}"),
        (Language::Go, "package main\nfunc foo() {}"),
        (Language::Java, "class Foo { void bar() {} }"),
        (Language::Rust, "fn foo() {}"),
        (Language::CSharp, "class Foo { void Bar() {} }"),
        (Language::Ruby, "def foo; end"),
        (Language::Php, "<?php\nfunction foo() {}"),
    ];
    for (lang, source) in &test_cases {
        let parser = get_parser(*lang);
        let result = parser.parse_file(source, "test_file");
        assert!(
            result.is_ok(),
            "Parser for {:?} should not fail on minimal source",
            lang
        );
    }
}

// ---------------------------------------------------------------------------
// Module-level call tracking (A1 fix)
// ---------------------------------------------------------------------------

/// JS parser must emit a synthetic <module> symbol so that module-level calls
/// have a valid caller_symbol_id when inserted into the calls table.
#[test]
fn test_js_parser_emits_module_symbol() {
    let parser = get_parser(Language::JavaScript);
    let source = std::fs::read_to_string("tests/fixtures/js_ts_repo/module_calls.js")
        .expect("fixture exists");
    let result = parser
        .parse_file(&source, "module_calls.js")
        .expect("parse succeeds");

    let module_sym = result.symbols.iter().find(|s| s.name == "<module>");
    assert!(
        module_sym.is_some(),
        "JS parser must emit a <module> symbol to anchor module-level calls"
    );
    let sym = module_sym.unwrap();
    assert_eq!(
        sym.kind,
        SymbolKind::Module,
        "<module> symbol must have kind=Module"
    );
    assert!(!sym.is_exported, "<module> symbol must not be exported");
}

/// Module-level calls must be emitted with caller_name = "<module>".
#[test]
fn test_js_module_level_calls_use_module_caller() {
    let parser = get_parser(Language::JavaScript);
    let source = std::fs::read_to_string("tests/fixtures/js_ts_repo/module_calls.js")
        .expect("fixture exists");
    let result = parser
        .parse_file(&source, "module_calls.js")
        .expect("parse succeeds");

    let module_calls: Vec<_> = result
        .calls
        .iter()
        .filter(|c| c.caller_name == "<module>")
        .collect();
    assert!(
        !module_calls.is_empty(),
        "module-level calls must have caller_name = '<module>'"
    );

    let calls_setup = result
        .calls
        .iter()
        .any(|c| c.callee_name == "setup" && c.caller_name == "<module>");
    assert!(
        calls_setup,
        "setup() at module level must be called from <module>"
    );
}

/// TS parser must also emit a <module> symbol.
#[test]
fn test_ts_parser_emits_module_symbol() {
    let parser = get_parser(Language::TypeScript);
    let source =
        std::fs::read_to_string("tests/fixtures/js_ts_repo/utils.ts").expect("fixture exists");
    let result = parser
        .parse_file(&source, "utils.ts")
        .expect("parse succeeds");

    let module_sym = result.symbols.iter().find(|s| s.name == "<module>");
    assert!(
        module_sym.is_some(),
        "TS parser must emit a <module> symbol to anchor module-level calls"
    );
}

// ---------------------------------------------------------------------------
// A2: JSX prop identifier tracking
// ---------------------------------------------------------------------------

#[test]
fn test_javascript_jsx_prop_bare_identifier_tracked_as_call() {
    let parser = get_parser(Language::JavaScript);
    let source = r#"function MyComponent() {
    function handleClick() {}
    return <Button onClick={handleClick} />;
}"#
    .to_string();
    let result = parser
        .parse_file(&source, "test.jsx")
        .expect("parse succeeds");
    let has_edge = result.calls.iter().any(|c| c.callee_name == "handleClick");
    assert!(
        has_edge,
        "Expected call edge to handleClick from JSX prop — got calls: {:?}",
        result
            .calls
            .iter()
            .map(|c| &c.callee_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_javascript_jsx_multiple_prop_identifiers_all_tracked() {
    let parser = get_parser(Language::JavaScript);
    let source = r#"function MyComponent() {
    function handleClick() {}
    function handleChange() {}
    return <Input onClick={handleClick} onChange={handleChange} />;
}"#
    .to_string();
    let result = parser
        .parse_file(&source, "test.jsx")
        .expect("parse succeeds");
    let has_click = result.calls.iter().any(|c| c.callee_name == "handleClick");
    let has_change = result.calls.iter().any(|c| c.callee_name == "handleChange");
    assert!(has_click, "Expected call edge to handleClick");
    assert!(has_change, "Expected call edge to handleChange");
}

#[test]
fn test_javascript_jsx_ternary_prop_not_tracked_as_identifier() {
    let parser = get_parser(Language::JavaScript);
    // Ternary expressions in props are deferred scope — should not produce
    // bare-identifier call edges from the jsx_expression→identifier path.
    let source = r#"function MyComponent() {
    function handleA() {}
    function handleB() {}
    return <Button onClick={condition ? handleA : handleB} />;
}"#
    .to_string();
    let result = parser
        .parse_file(&source, "test.jsx")
        .expect("parse succeeds");
    // The jsx_expression child of a ternary prop is a conditional_expression,
    // not a bare identifier — so A2 should NOT emit edges from this path.
    let ternary_edges = result
        .calls
        .iter()
        .filter(|c| c.callee_name == "handleA" || c.callee_name == "handleB")
        .count();
    assert_eq!(
        ternary_edges, 0,
        "Ternary prop values must not produce bare-identifier call edges (deferred scope)"
    );
}

// ---------------------------------------------------------------------------
// TSX grammar dispatch — .tsx files parse with LANGUAGE_TSX so the JSX
// extraction shipped for TypeScript (extract_jsx_call) is reachable.
// ---------------------------------------------------------------------------

#[test]
fn test_typescript_jsx_prop_bare_identifier_tracked_as_call() {
    // A2 twin of the JavaScript test above. Previously omitted because the
    // parser hardcoded LANGUAGE_TYPESCRIPT, which cannot parse JSX.
    let parser = get_parser(Language::TypeScript);
    let source = r#"function MyComponent() {
    function handleClick() {}
    return <Button onClick={handleClick} />;
}"#
    .to_string();
    let result = parser
        .parse_file(&source, "test.tsx")
        .expect("parse succeeds");
    let has_edge = result.calls.iter().any(|c| c.callee_name == "handleClick");
    assert!(
        has_edge,
        "Expected call edge to handleClick from TSX prop — got calls: {:?}",
        result
            .calls
            .iter()
            .map(|c| &c.callee_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_typescript_tsx_component_render_edge() {
    let parser = get_parser(Language::TypeScript);
    let source = r#"function Page() {
    return <Button label="go" />;
}"#
    .to_string();
    let result = parser
        .parse_file(&source, "page.tsx")
        .expect("parse succeeds");
    assert!(
        result.symbols.iter().any(|s| s.name == "Page"),
        "Expected Page symbol from .tsx source — got: {:?}",
        result.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    assert!(
        result.calls.iter().any(|c| c.callee_name == "Button"),
        "Expected component render edge to Button — got calls: {:?}",
        result
            .calls
            .iter()
            .map(|c| &c.callee_name)
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Syntax-error accounting — parse-health visibility across all 9 parsers
// ---------------------------------------------------------------------------

#[test]
fn test_typescript_type_assertion_still_parses_in_ts() {
    // .ts must stay on LANGUAGE_TYPESCRIPT: `<T>expr` type assertions are legal
    // TS but illegal TSX — the ambiguity upstream ships two grammars for.
    let parser = get_parser(Language::TypeScript);
    let source = r#"function convert(value: unknown): number {
    const n = <number>value;
    return n;
}"#;
    let result = parser
        .parse_file(source, "convert.ts")
        .expect("parse succeeds");
    assert!(result.symbols.iter().any(|s| s.name == "convert"));
    assert_eq!(
        result.syntax_error_count, 0,
        "angle-bracket type assertion must parse cleanly in .ts"
    );
}

#[test]
fn test_typescript_generic_arrow_still_parses_in_tsx() {
    // The trap case for the TSX grammar: generic arrows need `<T,>`.
    let parser = get_parser(Language::TypeScript);
    let source = r#"export const identity = <T,>(x: T): T => x;
function App() {
    return <Main />;
}"#;
    let result = parser
        .parse_file(source, "app.tsx")
        .expect("parse succeeds");
    assert_eq!(
        result.syntax_error_count, 0,
        "generic arrow with trailing comma must parse cleanly in .tsx"
    );
    assert!(result.calls.iter().any(|c| c.callee_name == "Main"));
}

#[test]
fn test_all_parsers_report_syntax_errors_on_garbage() {
    // parse_error_count == 0 must mean "clean parse", not "parser forgot the
    // wiring". Structural tripwire: a 10th parser that skips the shared
    // counter fails here.
    for lang in [
        Language::Python,
        Language::JavaScript,
        Language::TypeScript,
        Language::Go,
        Language::Java,
        Language::Rust,
        Language::CSharp,
        Language::Ruby,
        Language::Php,
    ] {
        // PHP treats bare text as inline HTML, so garbage must live inside
        // a <?php block to reach the parser proper.
        let garbage = match lang {
            Language::Php => "<?php ]]]] {{{{ ;;",
            _ => "]]]] this is not valid syntax <<<<< ;;;",
        };
        let parser = get_parser(lang);
        let ext = lang.extensions()[0];
        let result = parser
            .parse_file(garbage, &format!("garbage.{ext}"))
            .expect("error-tolerant parse returns Ok");
        assert!(
            result.syntax_error_count > 0,
            "{lang} parser should report syntax errors on garbage input"
        );
    }
}

#[test]
fn test_clean_source_reports_zero_syntax_errors() {
    let parser = get_parser(Language::TypeScript);
    let source = "export function add(a: number, b: number): number { return a + b; }";
    let result = parser
        .parse_file(source, "clean.ts")
        .expect("parse succeeds");
    assert_eq!(result.syntax_error_count, 0);
}

// ---------------------------------------------------------------------------
// Gap 4 — missing reference edge types (aliased refs, renamed imports,
// re-exports, const-only exports, type-only imports)
// ---------------------------------------------------------------------------

#[test]
fn test_ts_aliased_reference_emits_call_edge() {
    // `const sanitize = sanitizeForPrompt;` must produce a reference edge
    // (ParsedCall) from the alias to the target, or caller lists are floors.
    let parser = get_parser(Language::TypeScript);
    let source = "import { sanitizeForPrompt } from './sanitize';\n\
                  const sanitize = sanitizeForPrompt;\n\
                  export function run(x: string) { return sanitize(x); }\n";
    let result = parser.parse_file(source, "aliased.ts").expect("parse");
    assert!(
        result
            .calls
            .iter()
            .any(|c| c.callee_name == "sanitizeForPrompt" && c.caller_name == "sanitize"),
        "alias declarator must emit a call edge alias -> target, got {:?}",
        result.calls
    );
}

#[test]
fn test_ts_renamed_import_preserves_original_name() {
    // `import { helper as h }` — the local binding is `h`, but resolution
    // needs the exported name `helper` to find the symbol in the target file.
    let parser = get_parser(Language::TypeScript);
    let source = "import { helper as h } from './util';\nexport const use = () => h();\n";
    let result = parser.parse_file(source, "renamed.ts").expect("parse");
    let imp = result
        .imports
        .iter()
        .find(|i| i.imported_name == "h")
        .expect("local binding `h` must be the imported_name");
    assert_eq!(
        imp.original_name.as_deref(),
        Some("helper"),
        "renamed import must preserve the original exported name"
    );
}

#[test]
fn test_ts_reexport_creates_import_row_not_local_export() {
    // `export { a } from './b'` is a RE-export: it must create an import row
    // for dependency tracking and must NOT mark an unrelated local symbol
    // named `a` as exported.
    let parser = get_parser(Language::TypeScript);
    let source = "const a = 1;\nexport { a } from './b';\n";
    let result = parser.parse_file(source, "barrel.ts").expect("parse");
    assert!(
        result
            .imports
            .iter()
            .any(|i| i.imported_name == "a" && i.module_path == "./b" && !i.is_external),
        "re-export must create an import row for './b', got {:?}",
        result.imports
    );
    let local = result
        .symbols
        .iter()
        .find(|s| s.name == "a" && s.kind == SymbolKind::Constant)
        .expect("local const a exists");
    assert!(
        !local.is_exported,
        "re-export from another module must not mark the local `a` as exported"
    );
}

#[test]
fn test_ts_type_reexport_creates_import_row() {
    let parser = get_parser(Language::TypeScript);
    let source = "export type { T } from './types';\n";
    let result = parser.parse_file(source, "barrel.ts").expect("parse");
    assert!(
        result
            .imports
            .iter()
            .any(|i| i.imported_name == "T" && i.module_path == "./types"),
        "type re-export must create an import row, got {:?}",
        result.imports
    );
}

#[test]
fn test_ts_const_arrow_export_is_function_symbol() {
    // Pin: `export const f = (x) => ...` is a Function-kind exported symbol.
    let parser = get_parser(Language::TypeScript);
    let source = "export const scoreFidelity = (x: number) => x * 2;\n";
    let result = parser.parse_file(source, "score.ts").expect("parse");
    let sym = result
        .symbols
        .iter()
        .find(|s| s.name == "scoreFidelity")
        .expect("symbol exists");
    assert_eq!(
        sym.kind,
        SymbolKind::Function,
        "arrow const must be Function"
    );
    assert!(sym.is_exported);
}

#[test]
fn test_ts_satisfies_wrapped_arrow_is_function_symbol() {
    // `export const f = ((x) => x) satisfies Foo` — unwrap satisfies/as/parens
    // before deciding the symbol kind.
    let parser = get_parser(Language::TypeScript);
    let source = "type F = (x: number) => number;\n\
                  export const wrapped = ((x: number) => x) satisfies F;\n";
    let result = parser.parse_file(source, "wrapped.ts").expect("parse");
    let sym = result
        .symbols
        .iter()
        .find(|s| s.name == "wrapped")
        .expect("symbol exists");
    assert_eq!(
        sym.kind,
        SymbolKind::Function,
        "satisfies-wrapped arrow must be Function, got {:?}",
        sym.kind
    );
}

#[test]
fn test_ts_type_only_import_extracted() {
    // Pin: `import type { T }` lands in the imports table like a value import.
    let parser = get_parser(Language::TypeScript);
    let source =
        "import type { Config } from './config';\nexport const c: Config = {} as Config;\n";
    let result = parser.parse_file(source, "typed.ts").expect("parse");
    assert!(
        result
            .imports
            .iter()
            .any(|i| i.imported_name == "Config" && i.module_path == "./config"),
        "type-only import must be extracted, got {:?}",
        result.imports
    );
}

#[test]
fn test_ts_export_does_not_mark_nested_body_symbols_exported() {
    // `export function outer()` must export outer — NOT the locals inside its
    // body. Wrongly-exported locals become name-fallback resolution magnets.
    let parser = get_parser(Language::TypeScript);
    let source = "export function outer() {\n  const inner = 1;\n  return inner;\n}\n";
    let result = parser.parse_file(source, "scope.ts").expect("parse");
    let outer = result.symbols.iter().find(|s| s.name == "outer").unwrap();
    let inner = result.symbols.iter().find(|s| s.name == "inner").unwrap();
    assert!(outer.is_exported, "outer must be exported");
    assert!(
        !inner.is_exported,
        "function-body locals must NOT be marked exported"
    );
}

#[test]
fn test_js_export_does_not_mark_nested_body_symbols_exported() {
    let parser = get_parser(Language::JavaScript);
    let source = "export function outer() {\n  const inner = 1;\n  return inner;\n}\n";
    let result = parser.parse_file(source, "scope.js").expect("parse");
    let inner = result.symbols.iter().find(|s| s.name == "inner").unwrap();
    assert!(
        !inner.is_exported,
        "function-body locals must NOT be marked exported (JS)"
    );
}
