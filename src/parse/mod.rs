pub mod csharp;
pub mod go;
pub mod java;
pub mod javascript;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust_lang;
pub mod types;
pub mod typescript;

use types::{Language, ParseResult};

/// Count ERROR and MISSING nodes in a parse tree.
///
/// tree-sitter is error-tolerant: syntax it cannot parse becomes ERROR/MISSING
/// nodes inside an `Ok` tree, and symbols/calls under those nodes are silently
/// absent from extraction. Parsers store this count on `ParseResult` so
/// downstream consumers can judge whether graph answers for a file are
/// trustworthy. Clean trees exit in O(1) via `has_error()`; only error-bearing
/// subtrees are walked (`has_error()` is true for every ancestor of an error).
pub fn count_syntax_errors(node: tree_sitter::Node) -> usize {
    if !(node.has_error() || node.is_missing()) {
        return 0;
    }
    let mut count = usize::from(node.is_error() || node.is_missing());
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_syntax_errors(child);
    }
    count
}

/// Trait that all language parsers must implement.
pub trait LanguageParser: Send + Sync {
    fn language(&self) -> Language;
    fn parse_file(&self, source: &str, file_path: &str) -> anyhow::Result<ParseResult>;
}

/// Get a parser for the given language.
pub fn get_parser(lang: Language) -> Box<dyn LanguageParser> {
    match lang {
        Language::Python => Box::new(python::PythonParser::new()),
        Language::JavaScript => Box::new(javascript::JavaScriptParser::new()),
        Language::TypeScript => Box::new(typescript::TypeScriptParser::new()),
        Language::Go => Box::new(go::GoParser::new()),
        Language::Java => Box::new(java::JavaParser::new()),
        Language::Rust => Box::new(rust_lang::RustParser::new()),
        Language::CSharp => Box::new(csharp::CSharpParser::new()),
        Language::Ruby => Box::new(ruby::RubyParser::new()),
        Language::Php => Box::new(php::PhpParser::new()),
    }
}
