//! Count the number of recursive values.
//!
//! Test it with:
//!
//!     cargo run --example count <<< '{"a": [null, 1, "b"]}'
//!
//! This should yield `Ok(5)` to signify that it has counted 5 values.
//! (Object keys do not count.)

use hifijson::{Error, Expect, Lex};

/// Count the number of parsed values.
fn count<L: Lex>(next: u8, lexer: &mut L) -> Result<usize, hifijson::Error> {
    match next {
        b'a'..=b'z' => Ok(lexer.null_or_bool().map(|_| 1).ok_or(Expect::Value)?),
        b'0'..=b'9' => Ok(lexer.num_ignore().map(|_| 1)?),
        b'-' => Ok(lexer.discarded().num_ignore().map(|_| 1)?),
        b'"' => Ok(lexer.discarded().str_ignore().map(|_| 1)?),
        b'[' => {
            let mut sum = 1;
            lexer.discarded().seq(b']', L::ws_peek, |next, lexer| {
                sum += count(next, lexer)?;
                Ok::<_, hifijson::Error>(())
            })?;
            Ok(sum)
        }
        b'{' => {
            let mut sum = 1;
            lexer.discarded().seq(b'}', L::ws_peek, |next, lexer| {
                lexer.str_colon(next, L::ws_peek, |lexer| {
                    lexer.str_ignore().map_err(Error::Str)
                })?;
                sum += count(lexer.ws_peek().ok_or(Expect::Value)?, lexer)?;
                Ok::<_, hifijson::Error>(())
            })?;
            Ok(sum)
        }

        _ => Err(Expect::Value)?,
    }
}

fn process<L: Lex>(mut lexer: L) -> Result<usize, hifijson::Error> {
    lexer.exactly_one(L::ws_peek, |next, lexer| count(next, lexer))
}

fn main() {
    let filename = std::env::args().nth(1);
    let n = if let Some(filename) = filename {
        let file = std::fs::read_to_string(filename).expect("read file");
        process(hifijson::SliceLexer::new(file.as_bytes()))
    } else {
        use std::io::Read;
        process(hifijson::IterLexer::new(std::io::stdin().bytes()))
    };
    println!("{:?}", n)
}
