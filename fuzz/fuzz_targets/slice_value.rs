#![no_main]

use hifijson::{token::Lex, value, SliceLexer};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    SliceLexer::new(data).exactly_one(value::parse_unbounded);
});
