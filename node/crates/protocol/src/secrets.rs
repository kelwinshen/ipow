//! Keeps secrets out of the logs. An RPC endpoint often carries an API key
//! in its URL, and an HTTP library's error names the URL it failed to reach;
//! every error the node logs goes through `redact` first.

use std::fmt::Display;
use std::sync::RwLock;

static SECRETS: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// Registers values that must never appear in a log: endpoints and keys.
pub fn hide(value: &str) {
    let value = value.trim();
    // Too short to be a secret, and replacing it would garble the text.
    if value.len() >= 8 {
        SECRETS.write().unwrap().push(value.to_string());
    }
}

/// The text of `e` with every registered secret replaced.
pub fn redact(e: &dyn Display) -> String {
    let mut text = format!("{e:#}");
    for s in SECRETS.read().unwrap().iter() {
        text = text.replace(s.as_str(), "<hidden>");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_a_registered_endpoint() {
        hide("https://rpc.example/v1/abcdef123456");
        let e = anyhow::anyhow!("error sending request for url (https://rpc.example/v1/abcdef123456)");
        assert_eq!(redact(&e), "error sending request for url (<hidden>)");
    }
}
