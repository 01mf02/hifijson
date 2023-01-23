use core::num::NonZeroUsize;
use hifijson::value::{self, Value};
use hifijson::{validate, Error, IterLexer, SliceLexer};

fn bol<Num, Str>(b: bool) -> Value<Num, Str> {
    Value::Bool(b)
}

fn num<Num, Str>(n: Num, dot: Option<usize>, exp: Option<usize>) -> Value<Num, Str> {
    let dot = dot.map(|i| NonZeroUsize::new(i).unwrap());
    let exp = exp.map(|i| NonZeroUsize::new(i).unwrap());
    Value::Number((n, hifijson::num::Parts { dot, exp }))
}

fn int<Num, Str>(i: Num) -> Value<Num, Str> {
    num(i, None, None)
}

fn arr<const N: usize, Num, Str>(v: [Value<Num, Str>; N]) -> Value<Num, Str> {
    Value::Array(v.into())
}

fn obj<const N: usize, Num, Str>(v: [(Str, Value<Num, Str>); N]) -> Value<Num, Str> {
    Value::Object(v.into())
}

fn parses_to(slice: &[u8], v: Value<&str, &str>) -> Result<(), Error> {
    /*
    validate::exactly_one(&mut SliceLexer::new(slice))?;
    validate::exactly_one(&mut IterLexer::new(slice.iter().copied().map(Ok::<_, ()>)))?;
    */

    let parsed = value::exactly_one(&mut SliceLexer::new(slice))?;
    assert_eq!(parsed, v);

    let parsed = value::exactly_one(&mut IterLexer::new(slice.iter().copied().map(Ok::<_, ()>)))?;
    assert_eq!(parsed, v);

    Ok(())
}

#[test]
fn basic() -> Result<(), Error> {
    parses_to(b"null", Value::Null)?;
    parses_to(b"false", Value::Bool(false))?;
    parses_to(b"true", Value::Bool(true))?;
    Ok(())
}

#[test]
fn numbers() -> Result<(), Error> {
    parses_to(b"0", num("0", None, None))?;
    parses_to(b"-42", num("-42", None, None))?;
    parses_to(b"3.14", num("3.14", Some(1), None))?;

    // speed of light in m/s
    parses_to(b"299e6", num("299e6", None, Some(3)))?;
    // now a bit more precise
    parses_to(b"299.792e6", num("299.792e6", Some(3), Some(7)))?;

    Ok(())
}

#[test]
fn strings() -> Result<(), Error> {
    // greetings to Japan
    parses_to(r#""Hello 日本""#.as_bytes(), Value::String("Hello 日本"))?;
    Ok(())
}

#[test]
fn arrays() -> Result<(), Error> {
    parses_to(b"[]", arr([]))?;
    parses_to(b"[false, true]", arr([bol(false), bol(true)]))?;
    parses_to(b"[0, 1]", arr([int("0"), int("1")]))?;
    parses_to(b"[[]]", arr([arr([])]))?;

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

    Ok(())
}
