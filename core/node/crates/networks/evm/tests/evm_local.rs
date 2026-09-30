//! Runs the EVM adapter against a real local network: the shared suite of
//! `ipow-testing`, on Hardhat.

use ipow_network_evm::testing::EvmWorld;
use ipow_testing::suite;

macro_rules! on_hardhat {
    ($($name:ident),* $(,)?) => {$(
        #[tokio::test]
        async fn $name() {
            suite::$name(&EvmWorld::new().await).await;
        }
    )*};
}

on_hardhat!(
    reads_jobs_bonds_and_time_and_bids,
    carries_a_job_from_bid_to_payment,
    slashes_a_missed_duty,
    jumps_and_extends_back,
    settles_a_fork_challenge_by_work,
    slashes_an_unanswered_parent_question,
);
