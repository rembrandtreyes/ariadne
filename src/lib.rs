pub mod config;
pub mod parse;
pub mod db;
pub mod pipeline;
pub mod graph;
pub mod analysis;
pub mod search;
pub mod output;
pub mod mcp;
pub mod dashboard;

#[cfg(feature = "lsp")]
pub mod lsp;

#[cfg(feature = "watch")]
pub mod watch;

pub mod plugins;
