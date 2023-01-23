# High-fidelity JSON lexer and parser

`hifijson` is a Rust crate that provides a high-fidelity JSON lexer and parser.
In this context, high-fidelity means that unlike many other parsers,
`hifijson` aims to preserve input data very faithfully, in particular numbers.

* No dependencies
* `no_std`, `alloc` optional
* Support for both reading from slices and from byte iterators:
  This is important if you are writing an application that should
  read from files as well as from standard input, for example.
* Performance
* Portability
* Mostly zero-copy deserialisation:
  Due to the presence of escaped characters in JSON strings,
  full zero-copy deserialisation of JSON data is not possible.
  However, `hifijson` attempts to minimise allocations in presence of strings.


## Comparison to `serde_json`

`serde_json` is currently the most popular JSON parser written in Rust.
However, there are some deficiencies of `serde_json`:

* Numbers can be parsed with arbitrary precision
  (via the feature flag `arbitrary_precision`),
  but they cannot be deserialised (by implementing the `Deserialize` trait)
  to anything else than a `serde_json::Value`
  [#896](https://github.com/serde-rs/json/issues/896).
  Instead, one has to deserialize to `serde_json::Value`,
  then convert that to something else, which costs time.
* When using `arbitrary_precision`, 
  `serde_json` incorrectly parses or rejects certain input;
  for example, it
  incorrectly  parses `{"$serde_json::private::Number": "1.0"}` as number 1.0 and
  incorrectly rejects `{"$serde_json::private::Number": "foo"}`.
  I consider both of these to be bugs, but although they are known,
  the `serde_json` maintainers are
  ["fine sticking with this behaviour"](https://github.com/serde-rs/json/issues/826#issuecomment-1019360407).
* The behaviour of `serde_json` can be customised to some degree via feature flags.
  However, this is a relatively inflexible solution;
  for example, you can specify whether to preserve the order of
  keys in objects by using the `preserve_order` feature flag,
  but what happens when you have an object that contains the same key several times,
  for example `{"a": 1, "a": 2}`?
  Currently, `serde_json` parses this as `{"a": 2}`, silently discarding information.
  What if you would like to fail in this case?
  Well, you can just implement `Deserialize` yourself.
  Except ... that you cannot, if you are using `arbitrary_precision`.
  Ouch.

You should probably use `serde_json` if you want to
serialise / deserialise your existing Rust datatypes.
However, if you want to
process arbitrary JSON coming from the external world,
require some control over what kind of input you read, or
just care about fast build times and minimal dependencies,
then `hifijson` might be for you.

There is also [`serde-json-core`] for embedded usage of JSON;
however, this crate neither supports
arbitrary-precision numbers,
reading from byte iterators, nor
escape sequences in strings.


## Lexer

Writing a JSON parser is remarkably easy --- the hard part is actually lexing.
This is why `hifijson` provides you first and foremost with a lexer,
which you can then use to build a parser yourself.
Yes, you. You can do it.
`hifijson` tries to give you some basic abstractions to help you.
For example, the default parser is implemented in less than 40 lines of code.


## Default parser

[Parsing JSON is a minefield](http://seriot.ch/projects/parsing_json.html),
because the JSON standard is underspecified or downright contradictory in certain aspects.
For this reason, a parser has to make certain decisions
which inputs to accept and which to reject.

`hifijson` comes with a default parser that might be good enough for many use cases.
This parser makes the following choices:

* Validation of strings:
  The parser validates that strings are valid UTF-8.
* Concatenation of JSON values:
  Many JSON processing tools accept multiple root JSON values in a JSON file.
  For example, `[] 42 true {"a": "b"}`.
  However, defining formally what these tools actually accept or reject is not simple.
  For example, `serde_json` accepts `[]"a"`, but it rejects `42"a"`.
  The default behaviour of this parser is to accept any concatenation of
  `JSON-text` (as defined in [RFC 8259]) that can be somehow reconstructed.
  This allows for weird-looking things like `nulltruefalse`, `1.0"a"`,
  but some values cannot be reconstructed, such as `1.042.0`, because this may be either
  a concatenation of `1.0` and `42.0` or
  a concatenation of `1.04` and `2.0`.
  In that sense, `hifijson` attempts to implement a policy that is
  as permissive and easily describable as possible.

Furthermore, the parser passes all tests of the
[JSON parsing test suite](https://github.com/nst/JSONTestSuite).


## Fuzzing

To run the fuzzer, [install `cargo-fuzz`](https://rust-fuzz.github.io/book/cargo-fuzz/setup.html).
Then, if you do not wish to use the nightly Rust compiler as default,
run the fuzzer by `cargo +nightly fuzz run <target>`, where
`<target>` is any entry returned by `cargo +nightly fuzz list`.


[`serde-json-core`]: https://github.com/rust-embedded-community/serde-json-core
[RFC 8259]: https://www.rfc-editor.org/rfc/rfc8259
