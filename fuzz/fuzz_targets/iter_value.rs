#![no_main]

use hifijson::{token::Lex, value, IterLexer};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    IterLexer::new(data.iter().copied().map(Ok::<_, ()>)).exactly_one(value::parse_unbounded);
});
