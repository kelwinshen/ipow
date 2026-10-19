//! The calls the node makes to each network's endpoint, counted, so it can
//! say how many it makes and an endpoint's plan is not used up unseen (on
//! 2026-10-11 the testnet node's 42 tasks, a round every 30 seconds, were
//! estimated at up to a million calls a day on one network). Each network's
//! client counts what it sends, by method; the node logs and clears the
//! counts now and then.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// Calls by network, then by method.
pub type Counts = BTreeMap<String, BTreeMap<String, u64>>;

static CALLS: OnceLock<Mutex<Counts>> = OnceLock::new();

fn calls() -> &'static Mutex<Counts> {
    CALLS.get_or_init(Default::default)
}

/// Counts `n` calls of `method` to `network`'s endpoint (a batch counts each
/// of its calls).
pub fn count(network: &str, method: &str, n: u64) {
    let mut c = calls().lock().unwrap_or_else(|e| e.into_inner());
    *c.entry(network.to_string()).or_default().entry(method.to_string()).or_default() += n;
}

/// The counts since the last take, by network and method, and clears them.
pub fn take() -> Counts {
    std::mem::take(&mut *calls().lock().unwrap_or_else(|e| e.into_inner()))
}

/// A network's counts as one line, the most called first:
/// `eth_call=120 eth_blockNumber=40`.
pub fn by_method(methods: &BTreeMap<String, u64>) -> String {
    let mut m: Vec<_> = methods.iter().collect();
    m.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    m.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_by_network_and_method_and_clears() {
        count("calls-test-a", "eth_call", 1);
        count("calls-test-a", "eth_call", 2);
        count("calls-test-a", "eth_getLogs", 4);
        count("calls-test-b", "getAccountInfo", 5);
        let t = take();
        assert_eq!(by_method(&t["calls-test-a"]), "eth_getLogs=4 eth_call=3");
        assert_eq!(t["calls-test-b"]["getAccountInfo"], 5);
        assert!(take().get("calls-test-a").is_none());
    }
}
