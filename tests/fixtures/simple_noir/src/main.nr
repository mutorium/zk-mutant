mod utils;

fn main(x: u64, y: pub u64) {
    assert(x < y);

    // ===========================================================================
    // If pubkey IS in tree: leaf != 0, proof generation fails
    // If pubkey NOT in tree: leaf == 0, proof succeeds
    // Also: a <= b && c >= d || e == f
    // ===========================================================================

    /*
    Block comment too:
    leaf != 0
    leaf == 0
    */

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
