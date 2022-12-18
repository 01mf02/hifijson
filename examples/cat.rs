//! JSON validator & pretty-printer.

use hifijson::{value, str, Error, IterLexer, LexAlloc, LexWrite, SliceLexer, Token};
use std::{fs, io};

#[derive(Default)]
struct Cli {
    parse: bool,
    many: bool,
    silent: bool,
}

fn process<L: LexAlloc>(cli: &Cli, lexer: &mut L) -> Result<(), Error> {
    if cli.parse {
        if cli.many {
            for v in value::many(lexer) {
                let v = v?;
                if !cli.silent {
                    println!("{v}")
                };
            }
        } else {
            let v = value::exactly_one(lexer)?;
            if !cli.silent {
                println!("{v}")
            };
        }
    } else {
        let mut seen = false;
        while let Some(token) = lexer.ws_token() {
            if seen && !cli.many {
                panic!("{:?}", Error::ExpectedEof);
            }
            if cli.silent {
                lex(lexer, token, &|_| ())?;
            } else {
                use std::io::Write;
                lex(lexer, token, &|b| io::stdout().write_all(b).unwrap())?;
                println!();
            }
            seen = true;
        }
        if !cli.many && !seen {
            panic!("{:?}", Error::ExpectedValue);
        }
    }
    Ok(())
}

fn lex<L: LexWrite>(lexer: &mut L, token: Token, print: &impl Fn(&[u8])) -> Result<(), Error> {
    match token {
        Token::Null => print(b"null"),
        Token::True => print(b"true"),
        Token::False => print(b"false"),
        Token::DigitOrMinus => {
            let mut num = Default::default();
            let _pos = lexer.num_bytes(&mut num)?;
            print(&num)
        }
        Token::Quote => lex_string(lexer, print)?,
        Token::LSquare => {
            print(b"[");
            let mut first = true;
            lexer.seq(Token::RSquare, |lexer, token| {
                if !core::mem::take(&mut first) {
                    print(b",");
                }
                lex(lexer, token, print)
            })?;
            print(b"]");
        }
        Token::LCurly => {
            print(b"{{");
            let mut first = true;
            lexer.seq(Token::RCurly, |lexer, token| {
                if !core::mem::take(&mut first) {
                    print(b",");
                }

                match token {
                    Token::Quote => lex_string(lexer, print)?,
                    _ => return Err(Error::ExpectedString),
                }

                if lexer.ws_token() != Some(Token::Colon) {
                    return Err(Error::ExpectedColon);
                }
                print(b":");

                match lexer.ws_token() {
                    Some(token) => lex(lexer, token, print),
                    _ => Err(Error::ExpectedValue),
                }
            })?;
            print(b"}}")
        }
        token => return Err(Error::Token(token)),
    }
    Ok(())
}

fn lex_string<L: LexWrite>(lexer: &mut L, print: &impl Fn(&[u8])) -> Result<(), str::Error> {
    print(b"\"");
    let mut bytes = L::Bytes::default();
    lexer.str_bytes(&mut bytes)?;
    print(&bytes);
    print(b"\"");
    Ok(())
}

fn process_file(cli: &Cli, path: &str) -> io::Result<()> {
    let file = fs::File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }?;
    process(cli, &mut SliceLexer::new(&mmap)).unwrap();
    Ok(())
}

fn process_stdin(cli: &Cli) -> io::Result<()> {
    use io::Read;
    process(cli, &mut IterLexer::new(io::stdin().bytes())).unwrap();
    Ok(())
}

fn main() -> io::Result<()> {
    let mut cli = Cli::default();
    let mut files = Vec::new();

    for arg in std::env::args().skip(1) {
        match &*arg {
            "--parse" => cli.parse = true,
            "--many" => cli.many = true,
            "--silent" => cli.silent = true,
            _ => files.push(arg),
        }
    }
    match &files[..] {
        [] => process_stdin(&cli),
        args => args.iter().try_for_each(|a| process_file(&cli, a)),
    }
}
