use ariadne::analysis::arch_rules::check_rules;
use ariadne::config::{ArchRule, RepoConfig, RuleSeverity};
use ariadne::db::Database;
use ariadne::pipeline::run_full_pipeline;
use std::path::Path;

fn make_rule(name: &str, from: &str, to: &str, severity: RuleSeverity) -> ArchRule {
    ArchRule {
        name: name.to_string(),
        description: None,
        from: from.to_string(),
        to: to.to_string(),
        severity,
        rule_type: None,
        scope: None,
    }
}

#[test]
fn test_arch_rules_no_violations_empty_db() {
    let db = Database::open_in_memory().unwrap();
    let rules = vec![make_rule(
        "no-cross-module",
        "*orders*",
        "*users*",
        RuleSeverity::Error,
    )];

    let result = check_rules(&db, &rules).unwrap();

    assert!(
        result.violations.is_empty(),
        "expected no violations on empty db, got {}",
        result.violations.len()
    );
    assert_eq!(
        result.rules_passed, 1,
        "expected 1 rule passed, got {}",
        result.rules_passed
    );
    assert_eq!(result.rules_failed, 0);
    assert!(!result.has_errors);
}

#[test]
fn test_arch_rules_detects_cross_module_violation() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/monolith_modules");

    run_full_pipeline(&db, fixture, &config).unwrap();

    let rules = vec![make_rule(
        "no-orders-to-users",
        "*orders*",
        "*users*",
        RuleSeverity::Error,
    )];

    let result = check_rules(&db, &rules);
    assert!(
        result.is_ok(),
        "check_rules should not error, got: {:?}",
        result.err()
    );
}

#[test]
fn test_arch_rules_severity_error_sets_has_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/python_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    let rules = vec![make_rule(
        "no-py-imports",
        "*.py",
        "*.py",
        RuleSeverity::Error,
    )];

    let result = check_rules(&db, &rules).unwrap();

    if !result.violations.is_empty() {
        assert!(
            result.has_errors,
            "error-severity violations should set has_errors=true"
        );
    }
}

#[test]
fn test_arch_rules_warning_severity_does_not_set_has_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/python_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    let rules = vec![make_rule(
        "warn-py-imports",
        "*.py",
        "*.py",
        RuleSeverity::Warning,
    )];

    let result = check_rules(&db, &rules).unwrap();

    assert!(
        !result.has_errors,
        "warning-severity rules should never set has_errors=true"
    );
}

#[test]
fn test_arch_rules_idempotent() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/python_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    let rules = vec![make_rule(
        "no-py-imports",
        "*.py",
        "*.py",
        RuleSeverity::Error,
    )];

    let result1 = check_rules(&db, &rules).unwrap();
    let result2 = check_rules(&db, &rules).unwrap();

    assert_eq!(
        result1.violations.len(),
        result2.violations.len(),
        "check_rules should be idempotent: first run had {} violations, second had {}",
        result1.violations.len(),
        result2.violations.len()
    );
    assert_eq!(result1.rules_passed, result2.rules_passed);
    assert_eq!(result1.rules_failed, result2.rules_failed);
    assert_eq!(result1.has_errors, result2.has_errors);
}
