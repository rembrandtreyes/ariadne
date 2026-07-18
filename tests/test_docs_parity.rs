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
/// `src/mcp/tools/mod.rs` — guarded by `test_list_tools_returns_full_surface`
/// in `tests/test_mcp_tools.rs`. When tools are added, update this constant,
/// `src/lib.rs` docstring, and the README "Available MCP Tools" table
/// in a single commit.
const EXPECTED_TOOL_COUNT: usize = 32;

/// Timed pipeline phases (discovery runs untimed before the transaction).
/// Guarded against executed truth by `test_pipeline_phase_count_matches_docs`.
const EXPECTED_PHASE_COUNT: usize = 15;

/// Every currently-shipped MCP↔REST mirror route. Shared by the CHANGELOG and
/// README invariants below so the docs cannot drift apart from each other.
/// When a new REST route ships, add it here and update both docs (plus
/// AGENT-GUIDE) in the same commit.
const REST_MIRRORS: &[&str] = &[
    "/api/entry_points",
    "/api/complexity_hotspots",
    "/api/god_objects",
    "/api/dependency_path",
    "/api/propose_edit_plan",
];

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
fn test_changelog_unreleased_mentions_current_tool_count() {
    // CHANGELOG.md is the third count-bearing doc on the front door (after
    // README and lib.rs docstring). When tools ship without a CHANGELOG bump,
    // the next release notes go out as a shipped lie. This test asserts the
    // [Unreleased] section names the current EXPECTED_TOOL_COUNT either as a
    // running total or as a `(N tools)` parenthetical.
    let changelog = read_repo_file("CHANGELOG.md");
    let start = changelog
        .find("## [Unreleased]")
        .expect("CHANGELOG should contain '## [Unreleased]' section");
    let section_after = &changelog[start..];
    let end = section_after[3..]
        .find("\n## ")
        .map(|i| i + 3)
        .unwrap_or(section_after.len());
    let unreleased = &section_after[..end];

    let total_now = format!("total now {}", EXPECTED_TOOL_COUNT);
    let parenthetical = format!("({} tools)", EXPECTED_TOOL_COUNT);
    let total_of = format!("total of {}", EXPECTED_TOOL_COUNT);

    assert!(
        unreleased.contains(&total_now)
            || unreleased.contains(&parenthetical)
            || unreleased.contains(&total_of),
        "CHANGELOG.md [Unreleased] should reference the current tool count \
         ({}) — expected one of `{}`, `{}`, or `{}`. Update CHANGELOG when \
         shipping new MCP tools so release notes don't go out as a shipped lie.",
        EXPECTED_TOOL_COUNT,
        total_now,
        parenthetical,
        total_of
    );
}

#[test]
fn test_changelog_unreleased_documents_recent_features() {
    // Every shipped MCP tool must appear in the [Unreleased] section before
    // the next release. This test asserts the most recent tools (those most
    // likely to be missed in a CHANGELOG bump) are documented. When this test
    // fails, add the tool to CHANGELOG.md [Unreleased]/Added in the same
    // commit that ships the tool.
    let changelog = read_repo_file("CHANGELOG.md");
    let start = changelog
        .find("## [Unreleased]")
        .expect("CHANGELOG should contain '## [Unreleased]' section");
    let section_after = &changelog[start..];
    let end = section_after[3..]
        .find("\n## ")
        .map(|i| i + 3)
        .unwrap_or(section_after.len());
    let unreleased = &section_after[..end];

    // Recent shipped MCP tools (verified against `all_tools()`). Extend this
    // slice when new tools ship that need a CHANGELOG-truth gate.
    let recent_tools: &[&str] = &["propose_edit_plan"];
    for tool in recent_tools {
        assert!(
            unreleased.contains(tool),
            "CHANGELOG.md [Unreleased] should document `{}` (shipped MCP tool). \
             Update CHANGELOG when shipping tools so the next release notes \
             accurately list new capabilities.",
            tool
        );
    }

    // Run #15 shipped 4 REST routes mirroring high-value MCP tools. The
    // CHANGELOG must announce REST parity progress so dashboard consumers
    // know which tools are HTTP-reachable.
    assert!(
        unreleased.to_lowercase().contains("rest") || unreleased.contains("/api/"),
        "CHANGELOG.md [Unreleased] should mention REST/dashboard routes — \
         shipped REST parity progress is invisible without it."
    );
}

#[test]
fn test_agent_guide_tool_count_matches_expected() {
    // AGENT-GUIDE.md is the highest-blast-radius doc — every MCP agent reads
    // the intro on first call. Tool-count drift here is the +1 drift pattern
    // that slipped past Runs #16-#18 (intro said "31-tool" while reality was
    // 32 after Run #16). Catch the stale value AND require the current one.
    let guide = read_repo_file("docs/AGENT-GUIDE.md");

    let stale_dash = format!("{}-tool", EXPECTED_TOOL_COUNT - 1);
    let stale_space = format!("{} tools", EXPECTED_TOOL_COUNT - 1);
    assert!(
        !guide.contains(&stale_dash) && !guide.contains(&stale_space),
        "AGENT-GUIDE.md contains stale tool count `{}` or `{}` — update to {} \
         when shipping new tools so agents don't land on a +1 drift on first call",
        stale_dash,
        stale_space,
        EXPECTED_TOOL_COUNT
    );

    let expected_dash = format!("{}-tool", EXPECTED_TOOL_COUNT);
    let expected_space = format!("{} tools", EXPECTED_TOOL_COUNT);
    assert!(
        guide.contains(&expected_dash) || guide.contains(&expected_space),
        "AGENT-GUIDE.md should reference current tool count ({}) — expected `{}` or `{}`",
        EXPECTED_TOOL_COUNT,
        expected_dash,
        expected_space
    );
}

#[test]
fn test_agent_guide_mentions_rest_routes() {
    // Run #15 shipped 4 REST routes; Run #18 shipped a 5th (`/api/propose_edit_plan`).
    // AGENT-GUIDE pre-Run-#19 had zero `/api/` mentions — agents using
    // Ariadne over the dashboard surface had no decision-tree guidance for
    // REST vs MCP. This test asserts the agent-facing docs include a REST
    // section once the surface is non-trivial.
    let guide = read_repo_file("docs/AGENT-GUIDE.md");
    assert!(
        guide.contains("/api/"),
        "AGENT-GUIDE.md should mention the `/api/` REST surface — there are 5+ \
         REST routes mirroring MCP tools but agent docs ignore them"
    );

    // At least 2 specific REST routes must appear so the doc names the
    // surface, not just gestures at it.
    let rest_routes: &[&str] = &[
        "/api/entry_points",
        "/api/complexity_hotspots",
        "/api/god_objects",
        "/api/dependency_path",
        "/api/propose_edit_plan",
    ];
    let hits: Vec<&str> = rest_routes
        .iter()
        .copied()
        .filter(|r| guide.contains(r))
        .collect();
    assert!(
        hits.len() >= 2,
        "AGENT-GUIDE.md should mention at least 2 specific REST routes by name; \
         found: {:?}",
        hits
    );
}

#[test]
fn test_changelog_rest_parity_lists_all_mirrors() {
    // CHANGELOG `[Unreleased]` "Dashboard REST parity" section pre-Run-#19
    // listed 4 routes from Run #15 but missed Run #18's
    // `/api/propose_edit_plan`. This test asserts every currently-shipped
    // REST mirror appears in the [Unreleased] section so the next release
    // notes don't go out as a shipped lie.
    let changelog = read_repo_file("CHANGELOG.md");
    let start = changelog
        .find("## [Unreleased]")
        .expect("CHANGELOG should contain '## [Unreleased]' section");
    let section_after = &changelog[start..];
    let end = section_after[3..]
        .find("\n## ")
        .map(|i| i + 3)
        .unwrap_or(section_after.len());
    let unreleased = &section_after[..end];

    for route in REST_MIRRORS {
        assert!(
            unreleased.contains(route),
            "CHANGELOG.md [Unreleased] should list `{}` — REST parity progress \
             is invisible without it. Update CHANGELOG when shipping new REST \
             routes so release notes accurately list new surfaces.",
            route
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

#[test]
fn test_readme_documents_rest_mirrors() {
    // CHANGELOG and AGENT-GUIDE document the REST mirrors (guarded above and
    // by test_agent_guide_mentions_rest_routes) but README — the front door —
    // documented zero REST routes. Every shipped mirror must appear in README
    // so the dashboard's HTTP surface is discoverable where users first look.
    let readme = read_repo_file("README.md");
    for route in REST_MIRRORS {
        assert!(
            readme.contains(route),
            "README.md should document the `{}` REST mirror — the dashboard's \
             HTTP surface is invisible at the front door without it",
            route
        );
    }

    // Parity-count invariant: the Web Dashboard section's REST table must have
    // exactly one row per shipped mirror — a 6th route added to the const
    // without a README table row (or vice versa) fails here.
    let start = readme
        .find("### Web Dashboard")
        .expect("README should contain '### Web Dashboard' heading");
    let section = &readme[start..];
    let end = section[4..]
        .find("\n### ")
        .or_else(|| section[4..].find("\n## "))
        .map(|i| i + 4)
        .unwrap_or(section.len());
    let dash_section = &section[..end];
    let table_rows = dash_section
        .lines()
        .filter(|line| line.starts_with("| `") && line.contains("/api/"))
        .count();
    assert_eq!(
        table_rows,
        REST_MIRRORS.len(),
        "README Web Dashboard REST table should have exactly {} rows (one per \
         shipped mirror in REST_MIRRORS)",
        REST_MIRRORS.len()
    );
}

#[test]
fn test_readme_file_summary_mentions_parse_error_count() {
    // get_file_summary's response includes parse_error_count (the per-file
    // parse-trust signal). The README table row is read as the tool's
    // contract — it must mention the field.
    let readme = read_repo_file("README.md");
    assert!(
        readme.contains("parse_error_count"),
        "README.md should mention `parse_error_count` — get_file_summary's \
         response exposes it as the per-file parse-trust signal"
    );
}

// ---------------------------------------------------------------------------
// Pipeline phase count — executed truth vs every doc that states the number
// ---------------------------------------------------------------------------

#[test]
fn test_pipeline_phase_count_matches_docs() {
    // Executed truth: the pipeline reports one PhaseTiming per timed phase.
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(dir.path().join("a.py"), "def alpha():\n    pass\n").unwrap();
    let db = ariadne::db::Database::open_in_memory().expect("in-memory db");
    let stats = ariadne::pipeline::run_full_pipeline(
        &db,
        dir.path(),
        &ariadne::config::RepoConfig::default(),
    )
    .expect("pipeline");
    assert_eq!(
        stats.phase_durations.len(),
        EXPECTED_PHASE_COUNT,
        "the pipeline ran {} timed phases but EXPECTED_PHASE_COUNT says {} — \
         update the constant AND every doc that states the number",
        stats.phase_durations.len(),
        EXPECTED_PHASE_COUNT
    );

    // Docs that state the number must agree with the executed count.
    let phase_label = format!("{EXPECTED_PHASE_COUNT}-phase");
    let stale_label = format!("{}-phase", EXPECTED_PHASE_COUNT - 1);
    for file in [
        "src/lib.rs",
        "src/pipeline/mod.rs",
        "docs/CONTEXT.md",
        "CLAUDE.md",
    ] {
        let content = read_repo_file(file);
        assert!(
            content.contains(&phase_label),
            "{file} must describe the pipeline as {phase_label}"
        );
        assert!(
            !content.contains(&stale_label),
            "{file} still says {stale_label} — phase-count drift"
        );
    }
}

// ---------------------------------------------------------------------------
// MCP runtime instructions — the get_info string ships to every agent session
// ---------------------------------------------------------------------------

#[test]
fn test_mcp_server_instructions_match_tool_count() {
    use rmcp::ServerHandler;

    let db = ariadne::db::Database::open_in_memory().expect("in-memory db");
    let service = ariadne::mcp::tools::AriadneService::new(db);
    let instructions = service
        .get_info()
        .instructions
        .expect("server should ship instructions");

    assert!(
        instructions.contains(&format!("{EXPECTED_TOOL_COUNT} tools total")),
        "get_info instructions must state the true tool count — agents plan \
         tool use from this string"
    );
    let stale = format!("{}-tool", EXPECTED_TOOL_COUNT - 1);
    assert!(
        !instructions.contains(&stale),
        "get_info instructions still reference `{stale}` — this string ships \
         to every agent session and was previously missed by doc scans that \
         only cover markdown files"
    );
}

// ---------------------------------------------------------------------------
// Project CLAUDE.md — the agent-facing structure map must not lie either
// ---------------------------------------------------------------------------

#[test]
fn test_claude_md_matches_tool_count() {
    let claude_md = read_repo_file("CLAUDE.md");
    assert!(
        claude_md.contains(&format!("{EXPECTED_TOOL_COUNT} tools")),
        "CLAUDE.md's structure map must state the true MCP tool count — it \
         previously said 10 while the server shipped 32"
    );
}

// ---------------------------------------------------------------------------
// README CLI truthfulness — every documented invocation must exist in the
// binary. Phantom flags (dead-code --threshold, watch --dash) shipped before;
// this pins every ```bash block to `--help` reality.
// ---------------------------------------------------------------------------

fn readme_bash_invocations() -> Vec<(String, Vec<String>)> {
    let readme = read_repo_file("README.md");
    let mut invocations = Vec::new();
    let mut in_bash = false;
    for line in readme.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_bash = trimmed == "```bash";
            continue;
        }
        if !in_bash || trimmed.starts_with('#') {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("ariadne ") else {
            continue;
        };
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        let Some(sub) = tokens.first() else { continue };
        if sub.starts_with('-') {
            continue;
        }
        let flags: Vec<String> = tokens
            .iter()
            .filter(|t| t.starts_with("--"))
            .map(|t| t.split('=').next().unwrap_or(t).to_string())
            .collect();
        invocations.push((sub.to_string(), flags));
    }
    invocations
}

#[test]
fn test_readme_cli_commands_and_flags_exist() {
    use std::collections::HashMap;
    use std::process::Command;

    let bin = env!("CARGO_BIN_EXE_ariadne");
    let invocations = readme_bash_invocations();
    assert!(
        !invocations.is_empty(),
        "README should document at least one `ariadne` invocation in a bash block"
    );

    let mut help_cache: HashMap<String, String> = HashMap::new();
    for (sub, flags) in invocations {
        let help = help_cache.entry(sub.clone()).or_insert_with(|| {
            let out = Command::new(bin)
                .args([sub.as_str(), "--help"])
                .output()
                .expect("failed to run ariadne --help");
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        });
        assert!(
            help.contains(&format!("Usage: ariadne {sub}")),
            "README documents `ariadne {sub}` but the binary does not accept \
             that subcommand — help said:\n{help}"
        );
        for flag in flags {
            assert!(
                help.contains(&flag),
                "README documents `ariadne {sub} {flag}` but `{flag}` is not \
                 in `ariadne {sub} --help` — phantom flags erode every other \
                 claim in the README"
            );
        }
    }
}
