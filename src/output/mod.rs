pub mod json;
pub mod mermaid;
pub mod text;

use serde::Serialize;

/// Output format selection for CLI results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Csv,
}

impl OutputFormat {
    pub fn from_flags(json: bool) -> Self {
        if json {
            Self::Json
        } else {
            Self::Text
        }
    }
}

/// Render a serializable value in the requested format.
pub fn render<T: Serialize + std::fmt::Debug>(
    value: &T,
    format: OutputFormat,
) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(value)?;
            Ok(json)
        }
        OutputFormat::Text => Ok(format!("{:#?}", value)),
        OutputFormat::Csv => Ok(format!("{:#?}", value)),
    }
}

/// Print a serializable value to stdout in the requested format.
pub fn print_output<T: Serialize + std::fmt::Debug>(
    value: &T,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let rendered = render(value, format)?;
    println!("{}", rendered);
    Ok(())
}
