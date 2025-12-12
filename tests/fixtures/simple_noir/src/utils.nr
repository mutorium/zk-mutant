// A small helper module to be imported by main.nr

pub fn check_addition(x: Field, y: Field) {
    assert(x + y == 3);
}

// Intentionally not called by any test.
// This gives zk-mutant at least one mutation site that should survive.
pub fn untested_comparison(a: Field, b: Field) {
    assert(a == b);
}

#[test]
fn test_check_addition() {
    check_addition(1, 2);
    // Uncomment to make test fail
    // check_addition(2, 2);
}
