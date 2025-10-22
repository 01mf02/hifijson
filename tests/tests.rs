use core::num::NonZeroUsize;
use hifijson::token::Lex;
use hifijson::value::{self, Value};
use hifijson::{escape, ignore, num, str, Error, Expect, IterLexer, LexAlloc, SliceLexer};

fn boole<Num, Str>(b: bool) -> Value<Num, Str> {
    Value::Bool(b)
}

fn num<Num, Str>(n: Num, dot: Option<usize>, exp: Option<usize>) -> Value<Num, Str> {
    let dot = dot.map(|i| NonZeroUsize::new(i).unwrap());
    let exp = exp.map(|i| NonZeroUsize::new(i).unwrap());
    Value::Number((n, num::Parts { dot, exp }))
}

fn int<Num, Str>(i: Num) -> Value<Num, Str> {
    num(i, None, None)
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
        out.extend_from_slice(bytes);
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
    parses_to(b"0", int("0"))?;
    parses_to(b"42", int("42"))?;
    parses_to(b"-0", int("-0"))?;
    parses_to(b"-42", int("-42"))?;

    parses_to(b"3.14", num("3.14", Some(1), None))?;

    // speed of light in m/s
    parses_to(b"299e6", num("299e6", None, Some(3)))?;
    // now a bit more precise
    parses_to(b"299.792e6", num("299.792e6", Some(3), Some(7)))?;
    parses_to(b"-1.2e3", num("-1.2e3", Some(2), Some(4)))?;

    fails_with(b"-", num::Error::ExpectedDigit.into());

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
    parses_to(b"[0, 1]", arr([int("0"), int("1")]))?;
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
    parses_to(br#"{"a": 0}"#, obj([("a", int("0"))]))?;
    parses_to(
        br#"{"a": 0, "b": 1}"#,
        obj([("a", int("0")), ("b", int("1"))]),
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
