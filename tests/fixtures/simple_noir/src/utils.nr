// A small helper module to be imported by main.nr

pub fn check_addition(x: Field, y: Field) {
    assert(x + y == 3);
}

#[test]
fn test_check_addition() {
    check_addition(1, 2);
    // Uncomment to make test fail
    // check_addition(2, 2);
}
