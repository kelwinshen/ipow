//! Runs the Solana adapter against an in-process Solana with the programs:
//! the shared suite of `ipow-testing`, the same tests as on Hardhat.

use ipow_network_svm::testing::SvmWorld;
use ipow_testing::suite;

macro_rules! on_solana {
    ($($name:ident),* $(,)?) => {$(
        #[tokio::test]
        async fn $name() {
            suite::$name(&SvmWorld::new().await).await;
        }
    )*};
}

on_solana!(
    reads_jobs_bonds_and_time_and_bids,
    carries_a_job_from_bid_to_payment,
    slashes_a_missed_duty,
    jumps_and_extends_back,
    settles_a_fork_challenge_by_work,
    slashes_an_unanswered_parent_question,
);
