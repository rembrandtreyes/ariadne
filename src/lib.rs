//! # ariadne-graph
//!
//! Universal dependency graph for AI coding agents.
//!
//! Ariadne indexes a multi-language codebase into a SQLite graph database and exposes
//! the graph through an MCP server, LSP server, CLI, and REST dashboard. It supports
//! Python, JavaScript, TypeScript, Go, Java, Rust, C#, Ruby, and PHP.
//!
//! ## Key modules
//!
//! - [`pipeline`] — 14-phase indexing pipeline (parse → resolve → analyze)
//! - [`db`] — SQLite database wrapper and query layer
//! - [`mcp`] — Model Context Protocol server (22 tools)
//! - [`lsp`] — Language Server Protocol server (diagnostics, hover, code lens)
//! - [`graph`] — In-memory call graph, blast radius, call chain analysis
//! - [`analysis`] — Dead code detection, community detection, SCIP export
//! - [`search`] — Full-text and fuzzy symbol search

pub mod analysis;
pub mod commands;
pub mod config;
pub mod dashboard;
pub mod db;
pub mod graph;
pub mod mcp;
pub mod output;
pub mod parse;
pub mod pipeline;
pub mod search;

#[cfg(feature = "lsp")]
pub mod lsp;

#[cfg(feature = "watch")]
pub mod watch;

pub mod plugins;
