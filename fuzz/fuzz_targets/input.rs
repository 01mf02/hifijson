#![no_main]

use libfuzzer_sys::fuzz_target;

use hifijson::{value, SliceLexer};

fuzz_target!(|data: &[u8]| {

    value::exactly_one(&mut SliceLexer::new(data));

});
