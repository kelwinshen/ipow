//! The calls the node makes to each network's endpoint, counted, so it can
//! say how many it makes and an endpoint's plan is not used up unseen (on
//! 2026-10-11 the testnet node's 42 tasks, a round every 30 seconds, were
//! estimated at up to a million calls a day on one network). Each network's
//! client counts what it sends; the node logs and clears the counts now and
//! then.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

static CALLS: OnceLock<Mutex<BTreeMap<String, u64>>> = OnceLock::new();

fn calls() -> &'static Mutex<BTreeMap<String, u64>> {
    CALLS.get_or_init(Default::default)
}

/// Counts `n` calls to `network`'s endpoint (a batch counts each of its calls).
pub fn count(network: &str, n: u64) {
    let mut c = calls().lock().unwrap_or_else(|e| e.into_inner());
    *c.entry(network.to_string()).or_default() += n;
}

/// The counts since the last take, by network, and clears them.
pub fn take() -> BTreeMap<String, u64> {
    std::mem::take(&mut *calls().lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_by_network_and_clears() {
        count("calls-test-a", 1);
        count("calls-test-a", 2);
        count("calls-test-b", 5);
        let t = take();
        assert_eq!(t.get("calls-test-a"), Some(&3));
        assert_eq!(t.get("calls-test-b"), Some(&5));
        assert!(take().get("calls-test-a").is_none());
    }
}
