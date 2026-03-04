pub mod types;
pub mod python;
pub mod javascript;
pub mod typescript;
pub mod go;
pub mod java;
pub mod rust_lang;
pub mod csharp;
pub mod ruby;
pub mod php;

use types::{Language, ParseResult};

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
