#![no_main]

use hifijson::{value, IterLexer};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    value::exactly_one(&mut IterLexer::new(data.iter().copied().map(Ok::<_, ()>)));
});
