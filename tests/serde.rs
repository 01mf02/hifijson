#![cfg(feature = "serde")]

use hifijson::serde::from_slice;

#[test]
fn basic() {
    assert_eq!((), from_slice(b"null").unwrap());
    assert_eq!(true, from_slice(b"true").unwrap());
    assert_eq!(false, from_slice(b"false").unwrap());
}

#[test]
fn numbers() {
    assert_eq!(0, from_slice(b"0").unwrap());
    assert_eq!(42, from_slice(b"42").unwrap());
    assert_eq!(3.1415, from_slice(b"3.1415").unwrap());
    assert_eq!(-42, from_slice(b"-42").unwrap());
}

#[test]
fn strings() {
    assert_eq!("asdf", from_slice::<String>(br#""asdf""#).unwrap());
}

#[test]
fn arrays() {
    assert_eq!(Vec::<()>::new(), from_slice::<Vec<_>>(b"[]").unwrap());
    assert_eq!(vec![0], from_slice::<Vec<_>>(b"[0]").unwrap());
    assert_eq!(vec![0, 1], from_slice::<Vec<_>>(b"[0, 1]").unwrap());
}
