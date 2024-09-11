pub mod defi;
pub mod revm_utils;
pub mod abi;
pub mod utils;
pub mod prelude;


// RE-EXPORTS

// Alloy
pub use alloy_chains;
pub use alloy_primitives;
pub use alloy_signer;
pub use alloy_signer_local;
pub use alloy_provider;
pub use alloy_rpc_types;
pub use alloy_sol_types;
pub use alloy_transport;
pub use alloy_pubsub;
pub use alloy_network;
pub use alloy_contract;

// Revm
pub use revm;

pub const SUPPORTED_CHAINS: [u64; 5] = [1, 10, 56, 8453, 42161];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainId {
    Ethereum(u64),
    Optimism(u64),
    BinanceSmartChain(u64),
    Base(u64),
    Arbitrum(u64),
}

impl ChainId {

    pub fn new(id: u64) -> Self {
        match id {
            1 => ChainId::Ethereum(id),
            10 => ChainId::Optimism(id),
            56 => ChainId::BinanceSmartChain(id),
            8453 => ChainId::Base(id),
            42161 => ChainId::Arbitrum(id),
            _ => panic!("Unsupported chain id: {}", id),
        }
    }

    pub fn id(&self) -> u64 {
        match self {
            ChainId::Ethereum(id) => *id,
            ChainId::Optimism(id) => *id,
            ChainId::BinanceSmartChain(id) => *id,
            ChainId::Base(id) => *id,
            ChainId::Arbitrum(id) => *id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ChainId::Ethereum(_) => "Ethereum",
            ChainId::Optimism(_) => "Optimism",
            ChainId::BinanceSmartChain(_) => "Binance Smart Chain",
            ChainId::Base(_) => "Base",
            ChainId::Arbitrum(_) => "Arbitrum",
        }
    }
}