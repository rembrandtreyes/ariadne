use crate::db::Database;
use rusqlite::params;

/// Known built-in / runtime prefixes that will never resolve to user symbols.
const BUILTIN_PREFIXES: &[&str] = &[
    "console.",
    "window.",
    "document.",
    "navigator.",
    "location.",
    "history.",
    "Math.",
    "JSON.",
    "Object.",
    "Array.",
    "Promise.",
    "Number.",
    "String.",
    "Boolean.",
    "RegExp.",
    "Date.",
    "Map.",
    "Set.",
    "WeakMap.",
    "WeakSet.",
    "Symbol.",
    "Proxy.",
    "Reflect.",
    "Intl.",
    "Atomics.",
    "SharedArrayBuffer.",
    "ArrayBuffer.",
    "DataView.",
    "Float32Array.",
    "Float64Array.",
    "Int8Array.",
    "Int16Array.",
    "Int32Array.",
    "Uint8Array.",
    "process.",
    "Buffer.",
    "global.",
    "globalThis.",
    "Error.",
    "TypeError.",
    "RangeError.",
    "SyntaxError.",
    "ReferenceError.",
    "URL.",
    "URLSearchParams.",
    "Headers.",
    "Request.",
    "Response.",
    "TextEncoder.",
    "TextDecoder.",
    "AbortController.",
    "AbortSignal.",
    "FormData.",
    "Blob.",
    "File.",
    "FileReader.",
    "crypto.",
    "performance.",
    "queueMicrotask.",
    // Node.js built-in modules used as dotted
    "fs.",
    "path.",
    "os.",
    "util.",
    "http.",
    "https.",
    "net.",
    "dns.",
    "child_process.",
    "cluster.",
    "stream.",
    "zlib.",
    "events.",
];

const BUILTIN_NAMES: &[&str] = &[
    "setTimeout",
    "setInterval",
    "clearTimeout",
    "clearInterval",
    "requestAnimationFrame",
    "cancelAnimationFrame",
    "parseInt",
    "parseFloat",
    "isNaN",
    "isFinite",
    "encodeURI",
    "decodeURI",
    "encodeURIComponent",
    "decodeURIComponent",
    "fetch",
    "atob",
    "btoa",
    "structuredClone",
    "eval",
    "alert",
    "confirm",
    "prompt",
    "require",
    "import",
    "super",
    "this",
    // JS constructors used as functions
    "Number",
    "String",
    "Boolean",
    "Array",
    "Object",
    "Symbol",
    "BigInt",
    "Error",
    "TypeError",
    "RangeError",
    "SyntaxError",
    // Common Node.js globals
    "setImmediate",
    "clearImmediate",
    "queueMicrotask",
    // Rust standard library macros and functions
    "println",
    "eprintln",
    "print",
    "eprint",
    "format",
    "write",
    "writeln",
    "vec",
    "panic",
    "todo",
    "unimplemented",
    "unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "cfg",
    "env",
    "concat",
    "stringify",
    "include_str",
    "include_bytes",
    "line",
    "column",
    "file",
    "module_path",
    "dbg",
    "matches",
    // Rust std types used as constructors
    "Some",
    "None",
    "Ok",
    "Err",
    // Common Rust std trait methods and primitives
    "to_string",
    "to_owned",
    "clone",
    "into",
    "from",
    "as_ref",
    "as_mut",
    "as_str",
    "as_bytes",
    "unwrap",
    "unwrap_or",
    "unwrap_or_default",
    "unwrap_or_else",
    "expect",
    "is_empty",
    "is_some",
    "is_none",
    "is_ok",
    "is_err",
    "len",
    "push",
    "pop",
    "get",
    "insert",
    "remove",
    "contains",
    "iter",
    "map",
    "filter",
    "collect",
    "and_then",
    "or_else",
    "ok_or",
    "ok_or_else",
    "flat_map",
    "for_each",
    "any",
    "all",
    "find",
    "position",
    "enumerate",
    "zip",
    "take",
    "skip",
    "chain",
    "join",
    "split",
    "trim",
    "starts_with",
    "ends_with",
    "contains_key",
    "entry",
    "or_insert",
    "or_insert_with",
    "default",
];

fn is_builtin(name: &str) -> bool {
    // Strip trailing `!` for Rust macro calls (e.g., "format!" -> "format")
    let base = name.strip_suffix('!').unwrap_or(name);
    BUILTIN_NAMES.contains(&base)
        || BUILTIN_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Phase 5: Resolve call sites to their target symbols.
///
/// Multi-pass approach:
///   Pass 0 - Import-guided resolution       (confidence 0.98)
///   Pass 1 - Same-file exact match           (confidence 0.95)
///   Pass 2 - Dotted expression resolution    (confidence 0.85-0.88)
///   Pass 3 - Same-service exact match        (confidence 0.75)
///   Pass 4 - Global exact match              (confidence 0.50)
///   Pass 5 - External import categorization  (confidence 0.90)
///   Pass 6 - Built-in categorization         (confidence 1.00)
///   Pass 7 - Local method call categorization (confidence 0.30)
pub fn resolve_calls(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();
    let unresolved = crate::db::RESOLUTION_UNRESOLVED;

    let mut pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 0: Import-guided resolution (confidence 0.98)
    // ------------------------------------------------------------------
    conn.execute_batch(
        "UPDATE calls SET callee_symbol_id = (
            SELECT s.id FROM imports i
            JOIN symbols s ON s.file_id = i.resolved_file_id
                          AND s.name = i.imported_name
            WHERE i.file_id = calls.file_id
              AND i.imported_name = calls.callee_name
              AND i.resolved_file_id IS NOT NULL
            LIMIT 1
         ), confidence = 0.98, resolution = 'import_guided'
         WHERE callee_symbol_id IS NULL
           AND EXISTS (
               SELECT 1 FROM imports i
               JOIN symbols s ON s.file_id = i.resolved_file_id
                             AND s.name = i.imported_name
               WHERE i.file_id = calls.file_id
                 AND i.imported_name = calls.callee_name
                 AND i.resolved_file_id IS NOT NULL
           )",
    )?;
    tracing::info!("call_resolution pass_0_import: {}ms", pass_start.elapsed().as_millis());
    pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 1: Exact match on name within same file (confidence 0.95)
    // ------------------------------------------------------------------
    conn.execute_batch(
        "UPDATE calls SET callee_symbol_id = (
            SELECT s.id FROM symbols s
            WHERE s.name = calls.callee_name
              AND s.file_id = calls.file_id
            LIMIT 1
         ), confidence = 0.95, resolution = 'same_file'
         WHERE callee_symbol_id IS NULL
           AND EXISTS (
               SELECT 1 FROM symbols s
               WHERE s.name = calls.callee_name
                 AND s.file_id = calls.file_id
           )",
    )?;
    tracing::info!("call_resolution pass_1_same_file: {}ms", pass_start.elapsed().as_millis());
    pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 2: Dotted expression resolution (confidence 0.85-0.88)
    // ------------------------------------------------------------------
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT c.id, c.callee_name, c.file_id FROM calls c
                 WHERE c.callee_symbol_id IS NULL AND c.callee_name LIKE '%.%'
                   AND c.resolution = '{}'",
            crate::db::RESOLUTION_UNRESOLVED,
        ))?;

        let dotted_calls: Vec<(i64, String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut update_stmt = conn.prepare(
            "UPDATE calls SET callee_symbol_id = ?1, confidence = ?2, resolution = ?3
             WHERE id = ?4",
        )?;

        for (call_id, callee_name, file_id) in &dotted_calls {
            let method_name = match callee_name.rsplit('.').next() {
                Some(m) if !m.is_empty() => m,
                _ => continue,
            };

            // 2a: Same-file match on extracted method name
            let found: Option<i64> = conn
                .query_row(
                    "SELECT id FROM symbols WHERE name = ?1 AND file_id = ?2 LIMIT 1",
                    params![method_name, file_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(sym_id) = found {
                update_stmt.execute(params![sym_id, 0.85, "dotted_same_file", call_id])?;
                continue;
            }

            // 2b: Import-guided for the object portion
            let parts: Vec<&str> = callee_name.splitn(2, '.').collect();
            if parts.len() == 2 {
                let obj_name = parts[0];
                let found: Option<i64> = conn
                    .query_row(
                        "SELECT s.id FROM imports i
                         JOIN symbols s ON s.file_id = i.resolved_file_id AND s.name = ?1
                         WHERE i.file_id = ?2 AND i.imported_name = ?3
                         AND i.resolved_file_id IS NOT NULL
                         LIMIT 1",
                        params![method_name, file_id, obj_name],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(sym_id) = found {
                    update_stmt.execute(params![sym_id, 0.88, "dotted_import_guided", call_id])?;
                    continue;
                }
            }

            // 2c: Same-service match on extracted method name
            let found: Option<i64> = conn
                .query_row(
                    "SELECT s.id FROM symbols s
                     JOIN files f ON s.file_id = f.id
                     JOIN files cf ON cf.id = ?2
                     WHERE s.name = ?1 AND f.service_id = cf.service_id
                     LIMIT 1",
                    params![method_name, file_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(sym_id) = found {
                update_stmt.execute(params![sym_id, 0.65, "dotted_same_service", call_id])?;
            }
        }
    }
    tracing::info!("call_resolution pass_2_dotted: {}ms", pass_start.elapsed().as_millis());
    pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 3: Exact match on name within same service (confidence 0.75)
    // ------------------------------------------------------------------
    conn.execute_batch(&format!(
        "UPDATE calls SET callee_symbol_id = (
                SELECT s.id FROM symbols s
                JOIN files f ON s.file_id = f.id
                JOIN files cf ON calls.file_id = cf.id
                WHERE s.name = calls.callee_name
                  AND f.service_id = cf.service_id
                LIMIT 1
             ), confidence = 0.75, resolution = 'same_service'
             WHERE callee_symbol_id IS NULL
               AND resolution = '{unresolved}'
               AND EXISTS (
                   SELECT 1 FROM symbols s
                   JOIN files f ON s.file_id = f.id
                   JOIN files cf ON calls.file_id = cf.id
                   WHERE s.name = calls.callee_name
                     AND f.service_id = cf.service_id
               )",
    ))?;
    tracing::info!("call_resolution pass_3_same_service: {}ms", pass_start.elapsed().as_millis());
    pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 4: Global exact match (confidence 0.50)
    // ------------------------------------------------------------------
    conn.execute_batch(&format!(
        "UPDATE calls SET callee_symbol_id = (
                SELECT s.id FROM symbols s
                WHERE s.name = calls.callee_name
                LIMIT 1
             ), confidence = 0.5, resolution = 'global'
             WHERE callee_symbol_id IS NULL
               AND resolution = '{unresolved}'
               AND EXISTS (
                   SELECT 1 FROM symbols s
                   WHERE s.name = calls.callee_name
               )",
    ))?;
    tracing::info!("call_resolution pass_4_global: {}ms", pass_start.elapsed().as_millis());
    pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 5: External import categorization (confidence 0.90)
    //
    // Mark calls that match an external import as 'external' — the call
    // is to a known external package, just not to a local symbol.
    // This covers both direct matches and dotted matches (obj.method
    // where obj is an external import).
    // ------------------------------------------------------------------

    // 5a: Direct external import match
    conn.execute_batch(&format!(
        "UPDATE calls SET confidence = 0.90, resolution = 'external'
             WHERE callee_symbol_id IS NULL
               AND resolution = '{unresolved}'
               AND EXISTS (
                   SELECT 1 FROM imports i
                   WHERE i.file_id = calls.file_id
                     AND i.imported_name = calls.callee_name
                     AND i.is_external = 1
               )",
    ))?;

    // 5b: Dotted external import match (e.g., NextResponse.json where NextResponse is external)
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT c.id, c.callee_name, c.file_id FROM calls c
                 WHERE c.callee_symbol_id IS NULL AND c.resolution = '{unresolved}'
                   AND c.callee_name LIKE '%.%'",
        ))?;

        let dotted_unresolved: Vec<(i64, String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut update_stmt = conn
            .prepare("UPDATE calls SET confidence = 0.90, resolution = 'external' WHERE id = ?1")?;

        for (call_id, callee_name, file_id) in &dotted_unresolved {
            let obj_name = callee_name.split('.').next().unwrap_or(callee_name);
            let is_external: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM imports i
                     WHERE i.file_id = ?1 AND i.imported_name = ?2 AND i.is_external = 1",
                    params![file_id, obj_name],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if is_external {
                update_stmt.execute(params![call_id])?;
            }
        }
    }
    tracing::info!("call_resolution pass_5_external: {}ms", pass_start.elapsed().as_millis());
    pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 6: Built-in categorization (confidence 1.00)
    //
    // Mark remaining calls to known JS/Node built-ins.
    // ------------------------------------------------------------------
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT c.id, c.callee_name FROM calls c
                 WHERE c.callee_symbol_id IS NULL AND c.resolution = '{unresolved}'",
        ))?;

        let unresolved_calls: Vec<(i64, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut update_stmt = conn
            .prepare("UPDATE calls SET confidence = 1.0, resolution = 'builtin' WHERE id = ?1")?;

        for (call_id, callee_name) in &unresolved_calls {
            if is_builtin(callee_name) {
                update_stmt.execute(params![call_id])?;
            }
        }
    }
    tracing::info!("call_resolution pass_6_builtin: {}ms", pass_start.elapsed().as_millis());
    pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 7: Method call categorization (confidence 0.30)
    //
    // Mark ALL remaining dotted calls as method_call. Any dotted call
    // that survived passes 0-6 is a method call on a variable/object
    // that we can't statically resolve without type inference.
    // ------------------------------------------------------------------
    conn.execute_batch(&format!(
        "UPDATE calls SET confidence = 0.30, resolution = 'method_call'
             WHERE callee_symbol_id IS NULL AND resolution = '{unresolved}'
               AND callee_name LIKE '%.%'",
    ))?;
    tracing::info!("call_resolution pass_7_method_call: {}ms", pass_start.elapsed().as_millis());
    pass_start = std::time::Instant::now();

    // ------------------------------------------------------------------
    // Pass 8: React state setter pattern (confidence 0.40)
    //
    // Mark set[A-Z]* patterns as 'local' — these are React useState
    // destructured setters that will never match a declared symbol.
    // Also catches other common local patterns like resolve/reject.
    // ------------------------------------------------------------------
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT c.id, c.callee_name FROM calls c
                 WHERE c.callee_symbol_id IS NULL AND c.resolution = '{unresolved}'",
        ))?;

        let unresolved_calls: Vec<(i64, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut update_stmt =
            conn.prepare("UPDATE calls SET confidence = 0.40, resolution = 'local' WHERE id = ?1")?;

        for (call_id, callee_name) in &unresolved_calls {
            let is_react_setter = callee_name.starts_with("set")
                && callee_name.len() > 3
                && callee_name.chars().nth(3).is_some_and(|c| c.is_uppercase());
            let is_promise_callback = callee_name == "resolve" || callee_name == "reject";
            let is_callback = callee_name == "callback"
                || callee_name == "cb"
                || callee_name == "done"
                || callee_name == "next"
                || callee_name == "handler"
                || callee_name == "onSuccess"
                || callee_name == "onError"
                || callee_name == "onChange"
                || callee_name == "onClick"
                || callee_name == "onSubmit";
            let is_refetch =
                callee_name == "refetch" || callee_name == "mutate" || callee_name == "revalidate";
            // Catch on[A-Z]* callback props (onSave, onRegenerate, etc.)
            let is_on_handler = callee_name.starts_with("on")
                && callee_name.len() > 2
                && callee_name.chars().nth(2).is_some_and(|c| c.is_uppercase());

            if is_react_setter || is_promise_callback || is_callback || is_refetch || is_on_handler
            {
                update_stmt.execute(params![call_id])?;
            }
        }
    }
    tracing::info!("call_resolution pass_8_local_patterns: {}ms", pass_start.elapsed().as_millis());

    // ------------------------------------------------------------------
    // Report resolution statistics
    // ------------------------------------------------------------------
    let total_calls: i64 = conn.query_row("SELECT COUNT(*) FROM calls", [], |row| row.get(0))?;

    if total_calls == 0 {
        return Ok(());
    }

    let resolved_to_symbol: i64 = conn.query_row(
        "SELECT COUNT(*) FROM calls WHERE callee_symbol_id IS NOT NULL",
        [],
        |row| row.get(0),
    )?;

    let categorized: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM calls WHERE resolution != '{unresolved}'",),
        [],
        |row| row.get(0),
    )?;

    let truly_unresolved = total_calls - categorized;

    let pct_resolved = (resolved_to_symbol as f64 / total_calls as f64) * 100.0;
    let pct_categorized = (categorized as f64 / total_calls as f64) * 100.0;

    if truly_unresolved > 0 {
        eprintln!(
            "Call resolution: {resolved_to_symbol}/{total_calls} resolved ({pct_resolved:.0}%), \
             {pct_categorized:.0}% categorized, {truly_unresolved} unresolved"
        );
    } else {
        eprintln!(
            "Call resolution: {resolved_to_symbol}/{total_calls} resolved ({pct_resolved:.0}%), \
             {pct_categorized:.0}% categorized"
        );
    }

    Ok(())
}
