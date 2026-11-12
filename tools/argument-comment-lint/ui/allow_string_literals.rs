#![warn(uncommented_anonymous_literal_argument)]

fn describe(prefix: &str, suffix: &str) {
    let _ = (prefix, suffix);
}

fn main() {
    describe("openai", r"https://www.example.net/v1");
}
