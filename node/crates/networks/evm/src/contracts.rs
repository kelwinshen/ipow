//! Bindings of the new contracts, from their ABI files in `abi/`. The ABI
//! files are copied from `evm/artifacts`.

alloy::sol!(
    #[sol(rpc, all_derives)]
    IPoWProtocol,
    "abi/iPoWProtocol.json"
);

alloy::sol!(
    #[sol(rpc, all_derives)]
    IPoWLightClient,
    "abi/iPoWLightClient.json"
);

/// Conversion's ABI names the protocol's structs too; its own module keeps
/// them apart from the protocol's binding.
pub mod app {
    alloy::sol!(
        #[sol(rpc, all_derives)]
        Conversion,
        "abi/Conversion.json"
    );
}
pub use app::Conversion;

/// The vault's ABI names the protocol's structs too; its own module keeps
/// them apart.
pub mod vault {
    alloy::sol!(
        #[sol(rpc, all_derives)]
        IPoWVault,
        "abi/iPoWVault.json"
    );
}
pub use vault::IPoWVault;

/// The vault's two parts, and a receipt, each in its own module.
pub mod vault_home {
    alloy::sol!(
        #[sol(rpc, all_derives)]
        VaultHome,
        "abi/VaultHome.json"
    );
}
pub mod vault_receipts {
    alloy::sol!(
        #[sol(rpc, all_derives)]
        VaultReceipts,
        "abi/VaultReceipts.json"
    );
}
pub mod vault_receipt {
    alloy::sol!(
        #[sol(rpc, all_derives)]
        VaultReceipt,
        "abi/VaultReceipt.json"
    );
}
pub use vault_home::VaultHome;
pub use vault_receipt::VaultReceipt;
pub use vault_receipts::VaultReceipts;

alloy::sol! {
    #[sol(rpc)]
    /// `approve` is read as returning nothing: some tokens (USDT) return
    /// nothing, and a return value is not needed.
    interface IERC20 {
        function balanceOf(address owner) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external;
    }
}
