use hifijson::*;

/// Count the number of parsed values.
fn count<L: Lex>(token: Token, lexer: &mut L) -> Result<usize, hifijson::Error> {
    match token {
        Token::Other(b'a'..=b'z') => Ok(lexer.null_or_bool().map(|_| 1).ok_or(Expect::Value)?),
        Token::Other(b'0'..=b'9') => Ok(lexer.num_ignore().map(|_| 1)?),
        Token::Other(b'-') => Ok(lexer.discarded().num_ignore().map(|_| 1)?),
        Token::Quote => Ok(lexer.str_ignore().map(|_| 1)?),
        Token::LSquare => {
            let mut sum = 1;
            lexer.seq(Token::RSquare, L::ws_token, |token, lexer| {
                sum += count(token, lexer)?;
                Ok::<_, hifijson::Error>(())
            })?;
            Ok(sum)
        }
        Token::LCurly => {
            let mut sum = 1;
            lexer.seq(Token::RCurly, L::ws_token, |token, lexer| {
                lexer.str_colon(token, L::ws_token, |lexer| {
                    lexer.str_ignore().map_err(Error::Str)
                })?;
                sum += count(lexer.ws_token().ok_or(Expect::Value)?, lexer)?;
                Ok::<_, hifijson::Error>(())
            })?;
            Ok(sum)
        }

        _ => Err(Expect::Value)?,
    }
}

fn process<L: Lex>(mut lexer: L) -> Result<usize, hifijson::Error> {
    lexer.exactly_one(L::ws_token, |token, lexer| count(token, lexer))
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
