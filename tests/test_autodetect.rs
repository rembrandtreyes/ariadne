use ariadne::config::autodetect;
use std::path::Path;

#[test]
fn test_detect_current_repo() {
    let result = autodetect::detect(Path::new("."));
    // detect should return a valid result (may or may not have excluded paths)
    let _ = result.excluded_paths.len();
}
