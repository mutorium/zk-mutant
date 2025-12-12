mod utils;

fn main(x: u64, y: pub u64) {
    assert(x < y);
    // test comment
    assert(x != y);
    utils::check_addition(1, 2);
}

#[test]
fn test_main() {
    main(1, 2);
    // Uncomment to make test fail
    // main(1, 1);
}

#[test(should_fail)]
fn test_fail() {
    assert(false);
}
