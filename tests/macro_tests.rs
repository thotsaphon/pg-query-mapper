#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass_complex_struct.rs");
    t.compile_fail("tests/ui/fail_invalid_attribute.rs");
}
