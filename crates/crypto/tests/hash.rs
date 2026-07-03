use crypto::hash::hash_bytes;

#[test]
fn same_input_gives_same_hash() {
    let input = b"hello";
    assert_eq!(hash_bytes(input), hash_bytes(input));
}

#[test]
fn different_input_gives_different_hash() {
    let first = hash_bytes(b"hello");
    let second = hash_bytes(b"world");
    assert_ne!(first, second);
}

#[test]
fn empty_input_gives_stable_hash() {
    let first = hash_bytes(b"");
    let second = hash_bytes(b"");
    assert_eq!(first, second);
}
