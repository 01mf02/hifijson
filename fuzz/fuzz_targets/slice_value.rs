#![no_main]

use hifijson::{value, SliceLexer};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    value::exactly_one(&mut SliceLexer::new(data));
});
