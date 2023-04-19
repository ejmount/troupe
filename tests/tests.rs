#[test]
fn ux() {
    let t = trybuild::TestCases::new();
    t.pass("tests/successes/*.rs");
    t.compile_fail("tests/fails/*.rs");
}
