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
    assert!(!result.calls.is_empty(), "should find calls inside function bodies");
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
