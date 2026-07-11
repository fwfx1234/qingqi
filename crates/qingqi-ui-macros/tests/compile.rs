#[test]
fn validates_icon_names_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/valid.rs");
    cases.compile_fail("tests/ui/invalid.rs");
}
