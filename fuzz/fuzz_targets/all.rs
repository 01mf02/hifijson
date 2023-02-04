#![no_main]

use hifijson::{token::Lex, value, ignore, IterLexer, SliceLexer};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let iter = data.iter().copied().map(Ok::<_, ()>);
    let iv = IterLexer::new(iter).exactly_one(value::parse_unbounded);
    let sv = SliceLexer::new(data).exactly_one(value::parse_unbounded);
    match (&iv, &sv) {
        (Ok(i), Ok(s)) => assert_eq!(i, s),
        (Err(i), Err(s)) => assert_eq!(i, s),
        _ => panic!(),
    }

    let si = SliceLexer::new(data).exactly_one(ignore::parse);
    match (sv, si) {
        // ignore::parse does not validate UTF, so it is not critical if
        // value::parse fails with a UTF validation error and
        // ignore::parse does not
        (Err(hifijson::Error::Str(e)), _) if e.is_unicode_error() => (),
        (Ok(_), Ok(())) => (),
        (Err(v), Err(i)) => assert_eq!(v, i),
        _ => panic!(),
    }
});
