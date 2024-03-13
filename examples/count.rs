use hifijson::*;

/// Count the number of parsed values.
fn count(token: Token, lexer: &mut impl Lex) -> Result<usize, hifijson::Error> {
    match token {
        Token::Null | Token::True | Token::False => Ok(1),
        Token::DigitOrMinus => Ok(lexer.num_ignore().map(|_| 1)?),
        Token::Quote => Ok(lexer.str_ignore().map(|_| 1)?),
        Token::LSquare => {
            let mut sum = 1;
            lexer.seq(Token::RSquare, |token, lexer| {
                sum += count(token, lexer)?;
                Ok::<_, hifijson::Error>(())
            })?;
            Ok(sum)
        }
        Token::LCurly => {
            let mut sum = 1;
            lexer.seq(Token::RCurly, |token, lexer| {
                lexer.str_colon(token, |lexer| lexer.str_ignore().map_err(Error::Str))?;
                sum += count(lexer.ws_token().ok_or(Expect::Value)?, lexer)?;
                Ok::<_, hifijson::Error>(())
            })?;
            Ok(sum)
        }

        _ => Err(hifijson::Expect::Value)?,
    }
}

fn process(mut lexer: impl Lex) -> Result<usize, hifijson::Error> {
    lexer.exactly_one(|token, lexer| count(token, lexer))
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
