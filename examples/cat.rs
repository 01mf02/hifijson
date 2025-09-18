//! JSON validator & pretty-printer.

use core::ops::Deref;
use hifijson::{str, value, Error, Expect, IterLexer, LexAlloc, LexWrite, SliceLexer, Token};
use std::{fs, io};

#[derive(Default)]
struct Cli {
    parse: bool,
    many: bool,
    silent: bool,
    path: Option<String>,
}

#[derive(Debug, Default)]
struct PathElem {
    ints: Vec<usize>,
    strs: Vec<String>,
}

impl<Num: Deref<Target = str>, Str: Deref<Target = str>> TryFrom<value::Value<Num, Str>>
    for PathElem
{
    type Error = ();

    fn try_from(v: value::Value<Num, Str>) -> Result<Self, Self::Error> {
        let mut elem = Self::default();
        use value::{Sign::Pos, Value::*};
        match v {
            Array(arr) => {
                for x in arr {
                    match x {
                        Number(Pos, (n, parts)) if parts.is_int() => {
                            elem.ints.push(n.parse().unwrap())
                        }
                        String(s) => elem.strs.push(s.to_string()),
                        _ => todo!(),
                    }
                }
            }
            _ => todo!(),
        }
        Ok(elem)
    }
}

fn process<L: LexAlloc>(cli: &Cli, lexer: &mut L) -> Result<(), Error> {
    if cli.parse {
        if cli.many {
            let vs = core::iter::from_fn(|| Some(value::parse_unbounded(lexer.ws_token()?, lexer)));
            for v in vs {
                let v = v?;
                if !cli.silent {
                    println!("{}", v)
                };
            }
        } else {
            let v = lexer.exactly_one(L::ws_token, value::parse_unbounded)?;
            if !cli.silent {
                println!("{}", v)
            };
        }
    } else {
        let mut seen = false;
        while let Some(token) = lexer.ws_token() {
            if seen && !cli.many {
                Err(Expect::Eof)?
            }
            if cli.silent {
                lex(token, lexer, &|_| ())?;
            } else {
                let path: Vec<_> = cli.path.as_deref().map(parse_path).unwrap_or(Vec::new());
                use std::io::Write;
                filter(&path, token, lexer, &|b| io::stdout().write_all(b).unwrap())?;
            }
            seen = true;
        }
        if !cli.many && !seen {
            Err(Expect::Value)?
        }
    }
    Ok(())
}

fn filter<L: LexAlloc>(
    path: &[PathElem],
    token: Token,
    lexer: &mut L,
    print: &impl Fn(&[u8]),
) -> Result<(), Error> {
    let (elem, rest) = if let Some(path) = path.split_first() {
        path
    } else {
        lex(token, lexer, print)?;
        println!();
        return Ok(());
    };

    match token {
        Token::LSquare => {
            let mut idx = 0;
            lexer.seq(Token::RSquare, L::ws_token, |token, lexer| {
                let out = if elem.ints.is_empty() || elem.ints.contains(&idx) {
                    filter(rest, token, lexer, print)
                } else {
                    hifijson::ignore::parse(token, lexer)
                };
                idx += 1;
                out
            })?;
        }
        Token::LCurly => {
            let mut idx = 0;
            lexer.seq(Token::RCurly, L::ws_token, |token, lexer| {
                idx += 1;

                let key = lexer.str_colon(token, L::ws_token, |lexer| {
                    lexer.str_string().map_err(Error::Str)
                })?;
                let token = lexer.ws_token().ok_or(Expect::Value)?;
                if elem.strs.is_empty() || elem.strs.iter().any(|s| s == key.deref()) {
                    filter(rest, token, lexer, print)
                } else {
                    hifijson::ignore::parse(token, lexer)
                }
            })?;
        }
        _ => todo!(),
    }
    Ok(())
}

fn lex<L: LexWrite>(token: Token, lexer: &mut L, print: &impl Fn(&[u8])) -> Result<(), Error> {
    match token {
        Token::Other(b'a'..=b'z') => print(match lexer.null_or_bool().ok_or(Expect::Value)? {
            None => b"null",
            Some(true) => b"true",
            Some(false) => b"false",
        }),
        Token::Minus => {
            print(b"-");
            lex(Token::Other(b'0'), lexer, print)?
        }
        Token::Other(b'0'..=b'9') => {
            let mut num = Default::default();
            let _pos = lexer.num_bytes(&mut num)?;
            print(&num)
        }
        Token::Quote => lex_string(lexer, print)?,
        Token::LSquare => {
            print(b"[");
            let mut first = true;
            lexer.seq(Token::RSquare, L::ws_token, |token, lexer| {
                if !core::mem::take(&mut first) {
                    print(b",");
                }
                lex(token, lexer, print)
            })?;
            print(b"]");
        }
        Token::LCurly => {
            print(b"{");
            let mut first = true;
            lexer.seq(Token::RCurly, L::ws_token, |token, lexer| {
                if !core::mem::take(&mut first) {
                    print(b",");
                }

                lexer.str_colon(token, L::ws_token, |lexer| {
                    lex_string(lexer, print).map_err(Error::Str)
                })?;
                print(b":");
                lex(lexer.ws_token().ok_or(Expect::Value)?, lexer, print)
            })?;
            print(b"}")
        }
        _ => Err(Expect::Value)?,
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

/// Parse something like `[1]["a", "b"][]` to a path.
///
/// This is interpreted similarly to jq `.[1].["a", "b"].[]`.
fn parse_path(path: &str) -> Vec<PathElem> {
    use hifijson::token::Lex;
    let lexer = &mut SliceLexer::new(path.as_bytes());
    core::iter::from_fn(|| Some(value::parse_unbounded(lexer.ws_token()?, lexer)))
        .map(|e| PathElem::try_from(e.unwrap()).unwrap())
        .collect()
}

fn main() -> io::Result<()> {
    let mut cli = Cli::default();
    let mut files = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match &*arg {
            "--parse" => cli.parse = true,
            "--many" => cli.many = true,
            "--silent" => cli.silent = true,
            "--path" => cli.path = args.next(),
            _ => files.push(arg),
        }
    }
    match &files[..] {
        [] => process_stdin(&cli),
        args => args.iter().try_for_each(|a| process_file(&cli, a)),
    }
}
