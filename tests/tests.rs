use hifijson::token::Lex;
use hifijson::value::{self, Value};
use hifijson::{escape, ignore, num, str, Error, Expect, IterLexer, LexAlloc, Read, SliceLexer};

fn boole<Num, Str>(b: bool) -> Value<Num, Str> {
    Value::Bool(b)
}

fn num<Str>(n: &str) -> Value<&str, Str> {
    let parts = num::Parts {
        zero: n.starts_with("0") || n.starts_with("-0"),
        dot: n.contains('.'),
        exp: n.contains(['e', 'E']),
    };
    Value::Number((n, parts))
}

fn arr<Num, Str, const N: usize>(v: [Value<Num, Str>; N]) -> Value<Num, Str> {
    Value::Array(v.into())
}

fn obj<Num, Str, const N: usize>(v: [(Str, Value<Num, Str>); N]) -> Value<Num, Str> {
    Value::Object(v.into())
}

fn iter_of_slice(slice: &[u8]) -> impl Iterator<Item = Result<u8, ()>> + '_ {
    slice.iter().copied().map(Ok)
}

fn parses_to(slice: &[u8], v: Value<&str, &str>) -> Result<(), Error> {
    SliceLexer::new(slice).exactly_one(Lex::ws_peek, ignore::parse)?;
    IterLexer::new(iter_of_slice(slice)).exactly_one(Lex::ws_peek, ignore::parse)?;

    let parsed = SliceLexer::new(slice).exactly_one(Lex::ws_peek, value::parse_unbounded)?;
    assert_eq!(parsed, v);

    let parsed =
        IterLexer::new(iter_of_slice(slice)).exactly_one(Lex::ws_peek, value::parse_unbounded)?;
    assert_eq!(parsed, v);

    Ok(())
}

fn parses_to_binary_string(slice: &[u8], v: &[u8]) -> Result<(), Error> {
    SliceLexer::new(slice).exactly_one(Lex::ws_peek, ignore::parse)?;
    IterLexer::new(iter_of_slice(slice)).exactly_one(Lex::ws_peek, ignore::parse)?;

    let parsed = SliceLexer::new(slice).exactly_one(Lex::ws_peek, parse_binary_string)?;
    assert_eq!(parsed, v);

    let parsed =
        IterLexer::new(iter_of_slice(slice)).exactly_one(Lex::ws_peek, parse_binary_string)?;
    assert_eq!(parsed, v);

    Ok(())
}

fn parse_binary_string<L: LexAlloc>(next: u8, lexer: &mut L) -> Result<Vec<u8>, Error> {
    if next == b'"' {
        lexer.take_next();
    } else {
        Err(Error::Token(Expect::String))?
    }
    let on_string = |bytes: &mut L::Bytes, out: &mut Vec<u8>| {
        out.extend_from_slice(bytes.as_ref());
        Ok(())
    };
    let s = lexer.str_fold(Vec::new(), on_string, |lexer, out| {
        let next = lexer.take_next().ok_or(escape::Error::Eof)?;
        let c = lexer.escape(next).map_err(str::Error::Escape)?;
        out.extend(c.encode_utf8(&mut [0; 4]).as_bytes());
        Ok(())
    });
    s.map_err(Error::Str)
}

fn fails_with(slice: &[u8], e: Error) {
    let parsed = SliceLexer::new(slice).exactly_one(Lex::ws_peek, ignore::parse);
    assert_eq!(parsed.unwrap_err(), e);

    let parsed = IterLexer::new(iter_of_slice(slice)).exactly_one(Lex::ws_peek, ignore::parse);
    assert_eq!(parsed.unwrap_err(), e);

    parse_fails_with(slice, e)
}

fn parse_fails_with(slice: &[u8], e: Error) {
    let parsed = SliceLexer::new(slice).exactly_one(Lex::ws_peek, value::parse_unbounded);
    assert_eq!(parsed.unwrap_err(), e);

    let parsed =
        IterLexer::new(iter_of_slice(slice)).exactly_one(Lex::ws_peek, value::parse_unbounded);
    assert_eq!(parsed.unwrap_err(), e);
}

#[test]
fn basic() -> Result<(), Error> {
    parses_to(b"null", Value::Null)?;
    parses_to(b"false", Value::Bool(false))?;
    parses_to(b"true", Value::Bool(true))?;

    fails_with(b"nul", Expect::Value.into());
    fails_with(b"fal", Expect::Value.into());
    fails_with(b"t", Expect::Value.into());
    fails_with(b"a", Expect::Value.into());

    fails_with(b"true false", Expect::Eof.into());

    Ok(())
}

#[test]
fn numbers() -> Result<(), Error> {
    parses_to(b"0", num("0"))?;
    parses_to(b"42", num("42"))?;
    parses_to(b"-0", num("-0"))?;
    parses_to(b"-42", num("-42"))?;

    parses_to(b"3.14", num("3.14"))?;

    // speed of light in m/s
    parses_to(b"299e6", num("299e6"))?;
    // now a bit more precise
    parses_to(b"299.792e6", num("299.792e6"))?;
    parses_to(b"-1.2e+3", num("-1.2e+3"))?;
    parses_to(b"-1.2e-3", num("-1.2e-3"))?;

    parses_to(b"0.1e2", num("0.1e2"))?;
    parses_to(b"-0.1e2", num("-0.1e2"))?;

    fails_with(b"-", num::Error::ExpectedDigit.into());
    fails_with(b"1.", num::Error::ExpectedDigit.into());
    fails_with(b"1e", num::Error::ExpectedDigit.into());
    fails_with(b"1e+", num::Error::ExpectedDigit.into());
    fails_with(b"1e-", num::Error::ExpectedDigit.into());

    Ok(())
}

#[test]
fn strings() -> Result<(), Error> {
    // greetings to Japan
    parses_to(r#""Hello 日本""#.as_bytes(), Value::String("Hello 日本"))?;
    // single-character escape sequences
    parses_to(
        br#""\"\\\/\b\f\n\r\t""#,
        Value::String("\"\\/\u{8}\u{c}\n\r\t"),
    )?;

    // UTF-16 surrogate pairs
    parses_to(br#""\uD801\uDC37""#, Value::String("𐐷"))?;
    // the smallest value representable with a surrogate pair
    parses_to(br#""\ud800\udc00""#, Value::String("𐀀"))?;
    // the  largest value representable with a surrogate pair
    parses_to(br#""\udbff\udfff""#, Value::String("􏿿"))?;

    parses_to(br#""aa\nbb\ncc""#, Value::String("aa\nbb\ncc"))?;

    let escape = |e| Error::Str(str::Error::Escape(e));

    fails_with(br#""\X""#, escape(escape::Error::InvalidKind(b'X')));
    fails_with(br#""\U""#, escape(escape::Error::InvalidKind(b'U')));
    fails_with(br#""\"#, escape(escape::Error::Eof));
    fails_with(br#""\u00"#, escape(escape::Error::Eof));

    fails_with("\"\u{0}\"".as_bytes(), str::Error::Control.into());
    // corresponds to ASCII code 31 in decimal notation
    fails_with("\"\u{1F}\"".as_bytes(), str::Error::Control.into());
    fails_with(br#""abcd"#, str::Error::Eof.into());

    parse_fails_with(br#""\uDC37""#, escape(escape::Error::InvalidChar(0xdc37)));
    parse_fails_with(br#""\uD801""#, escape(escape::Error::ExpectedLowSurrogate));

    let s = [34, 159, 146, 150];
    let err = core::str::from_utf8(&s[1..]).unwrap_err();
    parse_fails_with(&s, str::Error::Utf8(err).into());

    Ok(())
}

#[test]
fn arrays() -> Result<(), Error> {
    parses_to(b"[]", arr([]))?;
    parses_to(b"[false, true]", arr([boole(false), boole(true)]))?;
    parses_to(b"[0, 1]", arr([num("0"), num("1")]))?;
    parses_to(b"[[]]", arr([arr([])]))?;

    fails_with(b"[", Expect::ValueOrEnd.into());
    fails_with(b"[1", Expect::CommaOrEnd.into());
    fails_with(b"[1 2", Expect::CommaOrEnd.into());
    fails_with(b"[1,", Expect::Value.into());

    Ok(())
}

#[test]
fn objects() -> Result<(), Error> {
    parses_to(b"{}", obj([]))?;
    parses_to(br#"{"a": 0}"#, obj([("a", num("0"))]))?;
    parses_to(
        br#"{"a": 0, "b": 1}"#,
        obj([("a", num("0")), ("b", num("1"))]),
    )?;

    fails_with(b"{", Expect::ValueOrEnd.into());
    fails_with(b"{0", Expect::String.into());
    fails_with(br#"{"a" 1"#, Expect::Colon.into());
    fails_with(br#"{"a": 1"#, Expect::CommaOrEnd.into());
    fails_with(br#"{"a": 1,"#, Expect::Value.into());

    Ok(())
}

#[test]
fn binary_strings() -> Result<(), Error> {
    parses_to_binary_string(br#""aaa\nbbb\nccc""#, b"aaa\nbbb\nccc")?;
    parses_to_binary_string(b"\"aaa\xffbbb\xffccc\"", b"aaa\xffbbb\xffccc")?;
    parses_to_binary_string(
        b"\"aaa\\u2200\xe2\x88\x80ccc\"",
        "aaa\u{2200}\u{2200}ccc".as_bytes(),
    )?;

    Ok(())
}

/// A custom error type that can hold both Expect and a custom variant.
#[derive(Debug, PartialEq, Eq)]
enum TryError {
    Token(Expect),
    Custom(&'static str),
}

impl From<Expect> for TryError {
    fn from(e: Expect) -> Self {
        TryError::Token(e)
    }
}

/// A fallible peek function that returns Err on '#' characters.
fn try_ws_peek<L: Lex>(lexer: &mut L) -> Result<Option<u8>, TryError> {
    lexer.eat_whitespace();
    match lexer.peek_next() {
        Some(b'#') => Err(TryError::Custom("comment not allowed")),
        other => Ok(other),
    }
}

fn try_parse(next: u8, lexer: &mut impl hifijson::Lex) -> Result<(), TryError> {
    ignore::parse(next, lexer).map_err(|e| match e {
        Error::Token(t) => TryError::Token(t),
        e => TryError::Custom(Box::leak(format!("{e}").into_boxed_str())),
    })
}

#[test]
fn try_exactly_one_ok() {
    let mut lexer = SliceLexer::new(b"true");
    let result: Result<(), TryError> = lexer.try_exactly_one(try_ws_peek, try_parse);
    assert!(result.is_ok());
}

#[test]
fn try_exactly_one_peek_error() {
    let mut lexer = SliceLexer::new(b"# comment");
    let result: Result<(), TryError> = lexer.try_exactly_one(try_ws_peek, try_parse);
    assert_eq!(result, Err(TryError::Custom("comment not allowed")));
}

#[test]
fn try_expect_ok() {
    let mut lexer = SliceLexer::new(b":value");
    let result: Result<Option<()>, TryError> =
        lexer.try_expect(|l| Ok(l.peek_next()), b':');
    assert_eq!(result, Ok(Some(())));
    // ':' should be consumed
    assert_eq!(lexer.peek_next(), Some(b'v'));
}

#[test]
fn try_expect_mismatch() {
    let mut lexer = SliceLexer::new(b"xvalue");
    let result: Result<Option<()>, TryError> =
        lexer.try_expect(|l| Ok(l.peek_next()), b':');
    assert_eq!(result, Ok(None));
    // 'x' should NOT be consumed
    assert_eq!(lexer.peek_next(), Some(b'x'));
}

#[test]
fn try_expect_peek_error() {
    let mut lexer = SliceLexer::new(b"# comment");
    let result: Result<Option<()>, TryError> =
        lexer.try_expect(try_ws_peek, b':');
    assert_eq!(result, Err(TryError::Custom("comment not allowed")));
}

#[test]
fn try_seq_ok() {
    // Parse sequence 1, 2, 3] using try_seq
    let mut lexer = SliceLexer::new(b"1, 2, 3]");
    let mut items = Vec::new();
    let result: Result<(), TryError> =
        lexer.try_seq(b']', |l| Ok(l.ws_peek()), |next, lexer| {
            items.push(next);
            ignore::parse(next, lexer).map_err(|e| match e {
                Error::Token(t) => TryError::Token(t),
                e => TryError::Custom(Box::leak(format!("{e}").into_boxed_str())),
            })
        });
    assert!(result.is_ok());
    assert_eq!(items.len(), 3);
}

#[test]
fn try_seq_empty() {
    let mut lexer = SliceLexer::new(b"]");
    let mut items: Vec<u8> = Vec::new();
    let result: Result<(), TryError> =
        lexer.try_seq(b']', |l| Ok(l.ws_peek()), |next, _lexer| {
            items.push(next);
            Ok(())
        });
    assert!(result.is_ok());
    assert!(items.is_empty());
}

#[test]
fn try_seq_peek_error() {
    let mut lexer = SliceLexer::new(b"1, # oops]");
    let mut count = 0;
    let result: Result<(), TryError> =
        lexer.try_seq(b']', try_ws_peek, |next, lexer| {
            count += 1;
            try_parse(next, lexer)
        });
    // The '#' should cause try_ws_peek to error after parsing "1,"
    assert_eq!(result, Err(TryError::Custom("comment not allowed")));
    assert_eq!(count, 1);
}

#[test]
fn try_seq_trailing_ok() {
    // Trailing comma: [1, 2,]
    let mut lexer = SliceLexer::new(b"1, 2,]");
    let mut count = 0;
    let result: Result<(), TryError> =
        lexer.try_seq_trailing(b']', |l| Ok(l.ws_peek()), |next, lexer| {
            count += 1;
            try_parse(next, lexer)
        });
    assert!(result.is_ok());
    assert_eq!(count, 2);
}

#[test]
fn try_seq_trailing_no_trailing() {
    // No trailing comma: [1, 2]
    let mut lexer = SliceLexer::new(b"1, 2]");
    let mut count = 0;
    let result: Result<(), TryError> =
        lexer.try_seq_trailing(b']', |l| Ok(l.ws_peek()), |next, lexer| {
            count += 1;
            try_parse(next, lexer)
        });
    assert!(result.is_ok());
    assert_eq!(count, 2);
}

#[test]
fn try_seq_trailing_empty() {
    let mut lexer = SliceLexer::new(b"]");
    let mut count = 0;
    let result: Result<(), TryError> =
        lexer.try_seq_trailing(b']', |l| Ok(l.ws_peek()), |_next, _lexer| {
            count += 1;
            Ok(())
        });
    assert!(result.is_ok());
    assert_eq!(count, 0);
}
