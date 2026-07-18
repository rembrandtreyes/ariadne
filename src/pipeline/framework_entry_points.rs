use crate::db::Database;
use rusqlite::params;
use std::path::Path;

/// A set of rules for identifying entry points in a specific framework.
struct FrameworkRuleSet {
    /// Framework identifier (matches autodetect output, e.g., "nextjs").
    framework: &'static str,
    /// Symbol names that are entry points when exported from any file.
    exported_names: &'static [&'static str],
    /// File path suffixes where ALL exported symbols are entry points.
    /// e.g., "page.tsx" matches "app/page.tsx" and "app/foo/page.tsx".
    entry_file_suffixes: &'static [&'static str],
    /// SQL LIKE patterns for symbol names that are always entry points.
    symbol_patterns: &'static [&'static str],
}

const NEXTJS_RULES: FrameworkRuleSet = FrameworkRuleSet {
    framework: "nextjs",
    exported_names: &[
        "GET",
        "POST",
        "PUT",
        "DELETE",
        "PATCH",
        "HEAD",
        "OPTIONS",
        "generateMetadata",
        "generateStaticParams",
        "getServerSideProps",
        "getStaticProps",
        "getStaticPaths",
        "middleware",
    ],
    entry_file_suffixes: &[
        "/page.tsx",
        "/page.ts",
        "/page.jsx",
        "/page.js",
        "/layout.tsx",
        "/layout.ts",
        "/layout.jsx",
        "/layout.js",
        "/loading.tsx",
        "/loading.ts",
        "/loading.jsx",
        "/loading.js",
        "/error.tsx",
        "/error.ts",
        "/error.jsx",
        "/error.js",
        "/not-found.tsx",
        "/not-found.ts",
        "/not-found.jsx",
        "/not-found.js",
        "/template.tsx",
        "/template.ts",
        "/template.jsx",
        "/template.js",
        "/default.tsx",
        "/default.ts",
        "/default.jsx",
        "/default.js",
        "/route.tsx",
        "/route.ts",
        "/route.jsx",
        "/route.js",
        "/global-error.tsx",
        "/global-error.ts",
        "/global-error.jsx",
        "/global-error.js",
    ],
    symbol_patterns: &[],
};

const NUXT_RULES: FrameworkRuleSet = FrameworkRuleSet {
    framework: "nuxt",
    exported_names: &[
        "defineNuxtConfig",
        "defineNuxtPlugin",
        "defineNuxtRouteMiddleware",
        "defineEventHandler",
        "defineNitroPlugin",
    ],
    entry_file_suffixes: &[],
    symbol_patterns: &[],
};

const ANGULAR_RULES: FrameworkRuleSet = FrameworkRuleSet {
    framework: "angular",
    exported_names: &[],
    entry_file_suffixes: &[],
    symbol_patterns: &[
        "%Component",
        "%Module",
        "%Service",
        "%Directive",
        "%Pipe",
        "%Guard",
    ],
};

const SVELTE_RULES: FrameworkRuleSet = FrameworkRuleSet {
    framework: "svelte",
    exported_names: &["load", "actions", "prerender", "ssr", "csr"],
    entry_file_suffixes: &[],
    symbol_patterns: &[],
};

/// Universal rules applied to all Node.js / JS / TS projects.
const NODE_RULES: FrameworkRuleSet = FrameworkRuleSet {
    framework: "node",
    exported_names: &[],
    entry_file_suffixes: &[],
    symbol_patterns: &["%Middleware", "%middleware"],
};

const ALL_RULE_SETS: &[&FrameworkRuleSet] = &[
    &NEXTJS_RULES,
    &NUXT_RULES,
    &ANGULAR_RULES,
    &SVELTE_RULES,
    &NODE_RULES,
];

/// Phase 6: Mark framework-detected entry points.
///
/// Uses detected frameworks to apply framework-specific rules that mark
/// additional symbols as entry points before dead code analysis runs.
pub fn apply_framework_rules(
    db: &Database,
    frameworks: &[String],
    root: &Path,
) -> anyhow::Result<()> {
    if frameworks.is_empty() {
        return Ok(());
    }

    let conn = db.conn();
    let mut total_marked = 0u64;

    for rule_set in ALL_RULE_SETS {
        if !frameworks.contains(&rule_set.framework.to_string()) {
            continue;
        }

        // Rule 1: Mark exported symbols with specific names as entry points.
        if !rule_set.exported_names.is_empty() {
            let placeholders: Vec<&str> = rule_set.exported_names.iter().map(|_| "?").collect();
            let sql = format!(
                "UPDATE symbols SET is_entry_point = 1
                 WHERE is_exported = 1 AND name IN ({})",
                placeholders.join(", ")
            );
            let mut stmt = conn.prepare(&sql)?;
            let params: Vec<&dyn rusqlite::types::ToSql> = rule_set
                .exported_names
                .iter()
                .map(|n| n as &dyn rusqlite::types::ToSql)
                .collect();
            let count = stmt.execute(params.as_slice())?;
            total_marked += count as u64;
        }

        // Rule 2: Mark all exported symbols from matching files as entry points.
        if !rule_set.entry_file_suffixes.is_empty() {
            let root_str = root.to_string_lossy();
            for suffix in rule_set.entry_file_suffixes {
                // Match files by path suffix — works with both relative and absolute paths.
                let like_pattern = format!("%{suffix}");
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM symbols s
                     JOIN files f ON s.file_id = f.id
                     WHERE s.is_exported = 1
                       AND (f.path LIKE ?1 OR f.absolute_path LIKE ?1)",
                    params![like_pattern],
                    |row| row.get(0),
                )?;
                if count > 0 {
                    conn.execute(
                        "UPDATE symbols SET is_entry_point = 1
                         WHERE is_exported = 1 AND file_id IN (
                             SELECT id FROM files
                             WHERE path LIKE ?1 OR absolute_path LIKE ?1
                         )",
                        params![like_pattern],
                    )?;
                    total_marked += count as u64;
                }
            }

            // Also mark non-exported functions in entry files as entry points
            // if they're the only function in the file (common for page/layout components)
            // that don't use explicit `export` keyword in some patterns.
            let _ = root_str; // suppress unused warning
        }

        // Rule 3: Mark symbols matching name patterns as entry points.
        for pattern in rule_set.symbol_patterns {
            let count = conn.execute(
                "UPDATE symbols SET is_entry_point = 1 WHERE name LIKE ?1",
                params![pattern],
            )?;
            total_marked += count as u64;
        }
    }

    if total_marked > 0 {
        eprintln!("Framework entry points: {total_marked} auto-detected from {frameworks:?}");
    }

    Ok(())
}

/// Detect frameworks from package.json dependencies in addition to config file markers.
///
/// Reads package.json at the given root and checks `dependencies` and
/// `devDependencies` for known framework packages.
pub fn detect_frameworks_from_manifest(root: &Path) -> Vec<String> {
    let mut frameworks = Vec::new();

    // Node.js: package.json
    let pkg_path = root.join("package.json");
    if let Ok(contents) = std::fs::read_to_string(&pkg_path) {
        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&contents) {
            let deps = pkg.get("dependencies").and_then(|v| v.as_object());
            let dev_deps = pkg.get("devDependencies").and_then(|v| v.as_object());

            let dep_names: Vec<&str> = deps
                .into_iter()
                .chain(dev_deps)
                .flat_map(|m| m.keys().map(|k| k.as_str()))
                .collect();

            let framework_deps: &[(&str, &str)] = &[
                ("next", "nextjs"),
                ("nuxt", "nuxt"),
                ("@angular/core", "angular"),
                ("@sveltejs/kit", "svelte"),
                ("vue", "vue"),
                ("express", "express"),
                ("fastify", "fastify"),
                ("@nestjs/core", "nestjs"),
                ("hono", "hono"),
                ("koa", "koa"),
                ("remix", "remix"),
                ("@remix-run/node", "remix"),
                ("astro", "astro"),
            ];

            for (dep_name, framework_name) in framework_deps {
                if dep_names.contains(dep_name) && !frameworks.contains(&framework_name.to_string())
                {
                    frameworks.push(framework_name.to_string());
                }
            }
        }
    }

    // Python: check for requirements.txt or pyproject.toml
    if root.join("requirements.txt").exists() {
        if let Ok(reqs) = std::fs::read_to_string(root.join("requirements.txt")) {
            if reqs
                .lines()
                .any(|l| l.trim().starts_with("flask") || l.trim().starts_with("Flask"))
            {
                frameworks.push("flask".to_string());
            }
            if reqs
                .lines()
                .any(|l| l.trim().starts_with("django") || l.trim().starts_with("Django"))
            {
                frameworks.push("django".to_string());
            }
            if reqs
                .lines()
                .any(|l| l.trim().starts_with("fastapi") || l.trim().starts_with("FastAPI"))
            {
                frameworks.push("fastapi".to_string());
            }
        }
    }

    frameworks
}
