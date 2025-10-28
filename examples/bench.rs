use std::time::Instant;

/// Append a binary tree of depth `d` to `s`.
fn binary(d: usize, s: &mut String) {
    s.push('[');
    if d > 0 {
        binary(d - 1, s);
        s.push(',');
        binary(d - 1, s);
    }
    s.push(']');
}

/// Create a JSON array that contains `n` repetitions of `s`.
fn many(s: &str, n: usize) -> String {
    let mut json = "[".to_string();
    json.push_str(s);
    for _ in 1..n {
        json.push(',');
        json.push_str(s);
    }
    json.push(']');
    json
}

fn main() {
    let mut tree = String::new();
    binary(23, &mut tree);

    const N: usize = 10_000_000;
    println!("Benchmark | Size | `serde_json` | `hifijson`");
    println!("- | -: | -: | -:");
    for (name, json) in [
        ("null", many("null", N)),
        ("pi", many("3.1415", N)),
        ("hello", many(r#""hello""#, N)),
        ("hello-world", many(r#""hello\nworld""#, N)),
        ("arr", many("[]", N)),
        ("tree", tree),
    ] {
        print!("{name}");
        print!(" | {} MiB", json.len() / 1024 / 1024);
        let now = Instant::now();
        serde(json.as_bytes());
        print!(" | {} ms", now.elapsed().as_millis());
        let now = Instant::now();
        hifi(json.as_bytes());
        print!(" | {} ms", now.elapsed().as_millis());
        println!();
    }
}

fn serde(s: &[u8]) {
    serde_json::from_slice::<serde_json::Value>(s).unwrap();
}

fn hifi(s: &[u8]) {
    use hifijson::token::Lex;
    let mut lexer = hifijson::SliceLexer::new(s);
    //lexer.exactly_one(hifijson::ignore::parse).unwrap();
    lexer
        .exactly_one(Lex::ws_peek, hifijson::value::parse_unbounded)
        .unwrap();
    //hifijson::serde::exactly_one::<serde_json::Value, _>(&mut lexer).unwrap();
}
