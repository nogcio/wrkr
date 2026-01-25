use std::collections::HashSet;

#[test]
fn luals_stub_files_are_unique_and_non_empty() {
    let files = wrkr_lua::luals_stub_files();
    assert!(!files.is_empty(), "expected stub files");

    let mut paths = HashSet::new();
    for f in files {
        assert!(
            paths.insert(f.path),
            "duplicate stub path found: {}",
            f.path
        );
        assert!(!f.contents.trim().is_empty(), "stub {} is empty", f.path);
    }
}
