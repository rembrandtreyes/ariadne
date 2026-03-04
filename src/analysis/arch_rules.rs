use crate::config::{ArchRule, RuleSeverity};
use crate::db::Database;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RuleViolation {
    pub rule_name: String,
    pub from_file: String,
    pub to_file: String,
    pub from_symbol: Option<String>,
    pub to_symbol: Option<String>,
    pub line: Option<u32>,
    pub severity: String,
}

#[derive(Debug, Serialize)]
pub struct RuleCheckResult {
    pub violations: Vec<RuleViolation>,
    pub rules_passed: usize,
    pub rules_failed: usize,
    pub has_errors: bool,
}

pub fn check_rules(db: &Database, rules: &[ArchRule]) -> anyhow::Result<RuleCheckResult> {
    let mut violations = Vec::new();
    let mut passed = 0;
    let conn = db.conn();

    for rule in rules {
        let rule_violations = check_single_rule(conn, rule)?;
        if rule_violations.is_empty() {
            passed += 1;
        } else {
            violations.extend(rule_violations);
        }
    }

    let failed = rules.len() - passed;
    let has_errors = violations.iter().any(|v| v.severity == "error");

    Ok(RuleCheckResult {
        violations,
        rules_passed: passed,
        rules_failed: failed,
        has_errors,
    })
}

fn check_single_rule(
    conn: &rusqlite::Connection,
    rule: &ArchRule,
) -> anyhow::Result<Vec<RuleViolation>> {
    let mut stmt = conn.prepare(
        "SELECT f1.path, f2.path, i.imported_name, i.line
         FROM imports i
         JOIN files f1 ON i.file_id = f1.id
         JOIN files f2 ON i.resolved_file_id = f2.id
         WHERE f1.path GLOB ?1 AND f2.path GLOB ?2",
    )?;

    let violations: Vec<RuleViolation> = stmt
        .query_map(rusqlite::params![rule.from, rule.to], |row| {
            Ok(RuleViolation {
                rule_name: rule.name.clone(),
                from_file: row.get(0)?,
                to_file: row.get(1)?,
                from_symbol: None,
                to_symbol: row.get::<_, Option<String>>(2)?,
                line: row.get(3)?,
                severity: match rule.severity {
                    RuleSeverity::Error => "error".to_string(),
                    RuleSeverity::Warning => "warning".to_string(),
                },
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(violations)
}
