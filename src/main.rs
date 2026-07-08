//! Example application entry point.

/// Program entry point.
fn main() {
    println!("{}", greeting("world"));
}

/// Build a greeting for the given name.
fn greeting(name: &str) -> String {
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// `greeting` includes the provided name.
    #[test]
    fn greeting_contains_name() {
        assert_eq!(greeting("world"), "Hello, world!");
    }
}
