use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Interface,
    Module,
    Variable,
    Constant,
    TypeAlias,
    Enum,
    Trait,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Method => write!(f, "method"),
            Self::Class => write!(f, "class"),
            Self::Interface => write!(f, "interface"),
            Self::Module => write!(f, "module"),
            Self::Variable => write!(f, "variable"),
            Self::Constant => write!(f, "constant"),
            Self::TypeAlias => write!(f, "type_alias"),
            Self::Enum => write!(f, "enum"),
            Self::Trait => write!(f, "trait"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSymbol {
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub line_start: u32,
    pub line_end: u32,
    pub is_exported: bool,
    pub signature: String,
    pub decorators: Vec<String>,
    pub parent_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedImport {
    pub imported_name: String,
    pub module_path: String,
    pub line: u32,
    pub is_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCall {
    pub caller_name: String,
    pub callee_name: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub method: String,
    pub path_pattern: String,
    pub handler_name: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCallSite {
    pub method: String,
    pub url_pattern: String,
    pub caller_name: String,
    pub line: u32,
    pub is_dynamic: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseResult {
    pub symbols: Vec<ParsedSymbol>,
    pub imports: Vec<ParsedImport>,
    pub calls: Vec<ParsedCall>,
    pub api_endpoints: Vec<ApiEndpoint>,
    pub api_calls: Vec<ApiCallSite>,
    /// ERROR/MISSING nodes tree-sitter produced for this file. Non-zero means
    /// extraction was partial: symbols and calls under error nodes are absent,
    /// so graph answers for this file may be incomplete.
    #[serde(default)]
    pub syntax_error_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    Rust,
    CSharp,
    Ruby,
    Php,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "py" => Some(Self::Python),
            "js" | "jsx" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "rs" => Some(Self::Rust),
            "cs" => Some(Self::CSharp),
            "rb" => Some(Self::Ruby),
            "php" => Some(Self::Php),
            _ => None,
        }
    }

    pub fn extensions(&self) -> &[&str] {
        match self {
            Self::Python => &["py"],
            Self::JavaScript => &["js", "jsx"],
            Self::TypeScript => &["ts", "tsx"],
            Self::Go => &["go"],
            Self::Java => &["java"],
            Self::Rust => &["rs"],
            Self::CSharp => &["cs"],
            Self::Ruby => &["rb"],
            Self::Php => &["php"],
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Java => "java",
            Self::Rust => "rust",
            Self::CSharp => "csharp",
            Self::Ruby => "ruby",
            Self::Php => "php",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
