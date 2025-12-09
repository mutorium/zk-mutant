fn main(x: u64, y: pub u64) {
    assert(x != y);
}

#[test]
fn test_main() {
    main(1, 2);

    // Uncomment to make test fail
    // main(1, 1);
}

#[test]
fn test_addition() {
    let x = 1;
    let y = 2;
    constrain x + y == 3;
}
