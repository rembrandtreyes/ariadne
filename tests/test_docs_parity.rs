//! Documentation parity tests — keep docs from drifting out of sync with code.
//!
//! Background: prior ProductForge runs repeatedly caught README/docstrings
//! that advertised the wrong MCP tool count (README said 10 tools, reality
//! was 31). These tests convert those lies into cargo-enforced invariants:
//! if someone adds tool #32 without updating the README and the lib.rs
//! docstring, `cargo test` fails loudly.
//!
//! Kept as integration tests (not unit) so they run against the shipped
//! source tree, not in-crate fixtures.

use std::fs;
use std::path::PathBuf;

/// Current MCP tool count. Must match `all_tools().len()` in
/// `src/mcp/tools/mod.rs` — guarded by `test_list_tools_returns_ten` in
/// `tests/test_mcp_tools.rs`. When tools are added, update this constant,
/// `src/lib.rs` docstring, and the README "Available MCP Tools" table
/// in a single commit.
const EXPECTED_TOOL_COUNT: usize = 32;

fn read_repo_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e))
}

#[test]
fn test_readme_mcp_table_matches_tool_count() {
    let readme = read_repo_file("README.md");
    let start = readme
        .find("### Available MCP Tools")
        .expect("README should contain '### Available MCP Tools' heading");
    let section = &readme[start..];
    // Take until the next top-level or second-level heading
    let end = section[4..]
        .find("\n## ")
        .or_else(|| section[4..].find("\n### "))
        .map(|i| i + 4)
        .unwrap_or(section.len());
    let table_section = &section[..end];

    let rows: Vec<&str> = table_section
        .lines()
        .filter(|line| line.starts_with("| `") && line.contains("` |"))
        .collect();

    assert_eq!(
        rows.len(),
        EXPECTED_TOOL_COUNT,
        "README 'Available MCP Tools' table should list {} tools; found {}",
        EXPECTED_TOOL_COUNT,
        rows.len()
    );
}

#[test]
fn test_lib_rs_docstring_has_correct_tool_count() {
    let lib_rs = read_repo_file("src/lib.rs");
    let expected = format!("({} tools)", EXPECTED_TOOL_COUNT);
    assert!(
        lib_rs.contains(&expected),
        "src/lib.rs docstring should contain literal `{}` — it drifts when \
         tools are added without updating the module doc comment",
        expected
    );
}

#[test]
fn test_agent_guide_exists_and_nonempty() {
    let guide = read_repo_file("docs/AGENT-GUIDE.md");
    assert!(
        guide.len() > 1000,
        "docs/AGENT-GUIDE.md should be > 1000 bytes (found: {})",
        guide.len()
    );
    // The guide must reference the onboarding triad so agents hitting it get
    // the happy path immediately, not a generic tool list.
    for onboarding_tool in ["get_entry_points", "get_god_objects", "blast_radius"] {
        assert!(
            guide.contains(onboarding_tool),
            "AGENT-GUIDE.md should reference `{}` in the onboarding decision tree",
            onboarding_tool
        );
    }
}

#[test]
fn test_readme_competitor_table_targets_current_threats() {
    // The README competitor table must mention at least 2 of the current
    // HIGH threats (from competitive landscape memory, as of 2026-04-24).
    // Stale comparison against dependency-cruiser / Axon alone is itself
    // a shipped strategic lie.
    let readme = read_repo_file("README.md");
    let current_threats = [
        "GitNexus",
        "Codebase-Memory",
        "Greptile",
        "Potpie",
        "Understand-Anything",
        "code-review-graph",
    ];
    let hits: Vec<&str> = current_threats
        .iter()
        .copied()
        .filter(|t| readme.contains(t))
        .collect();
    assert!(
        hits.len() >= 2,
        "README competitor table should name ≥2 current HIGH-threat competitors \
         ({:?}); found: {:?}",
        current_threats,
        hits
    );
}
