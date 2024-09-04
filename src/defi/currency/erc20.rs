use alloy_contract::private::Network;
use alloy_primitives::{Address, Bytes, U256};
use alloy_provider::Provider;
use alloy_rpc_types::BlockId;
use alloy_sol_types::SolCall;
use alloy_transport::Transport;

use crate::abi::erc20::ERC20;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio::try_join;

/// Represents an ERC20 Token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ERC20Token {
    pub chain_id: u64,
    pub address: Address,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub total_supply: U256,
    pub icon: Option<Vec<u8>>,
}

impl ERC20Token {
    pub async fn new<T, P, N>(
        client: P,
        address: Address,
        chain_id: u64,
    ) -> Result<Self, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        let symbol = Self::symbol(address, client.clone());
        let name = Self::name(address, client.clone());
        let decimals = Self::decimals(address, client.clone());
        let total_supply = Self::total_supply(address, client.clone());
        let (symbol, name, decimals, total_supply) =
            try_join!(symbol, name, decimals, total_supply)?;
        Ok(Self {
            chain_id,
            address,
            symbol,
            name,
            decimals,
            total_supply,
            icon: None,
        })
    }

    pub async fn balance_of<T, P, N>(
        &self,
        owner: Address,
        client: P,
        block_id: Option<BlockId>,
    ) -> Result<U256, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        let block = block_id.unwrap_or(BlockId::latest());
        let contract = ERC20::new(self.address, client);
        let b = contract.balanceOf(owner).block(block).call().await?;
        Ok(b.balance)
    }

    pub async fn allowance<T, P, N>(
        &self,
        owner: Address,
        spender: Address,
        client: P,
    ) -> Result<U256, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        let contract = ERC20::new(self.address, client);
        let allowance = contract.allowance(owner, spender).call().await?._0;
        Ok(allowance)
    }

    pub fn encode_balance_of(&self, owner: Address) -> Bytes {
        let contract = ERC20::balanceOfCall { owner };
        Bytes::from(contract.abi_encode())
    }

    pub fn encode_allowance(&self, owner: Address, spender: Address) -> Bytes {
        let contract = ERC20::allowanceCall { owner, spender };
        Bytes::from(contract.abi_encode())
    }

    pub fn encode_approve(&self, spender: Address, amount: U256) -> Bytes {
        let contract = ERC20::approveCall { spender, amount };
        Bytes::from(contract.abi_encode())
    }

    pub fn encode_transfer(&self, recipient: Address, amount: U256) -> Bytes {
        let contract = ERC20::transferCall { recipient, amount };
        Bytes::from(contract.abi_encode())
    }

    pub fn encode_deposit(&self) -> Bytes {
        let contract = ERC20::depositCall {};
        Bytes::from(contract.abi_encode())
    }

    pub fn encode_withdraw(&self, amount: U256) -> Bytes {
        let contract = ERC20::withdrawCall { amount };
        Bytes::from(contract.abi_encode())
    }

    pub fn decode_balance_of(&self, bytes: &Bytes) -> Result<U256, anyhow::Error> {
        let balance = ERC20::balanceOfCall::abi_decode_returns(&bytes, true)?;
        Ok(balance.balance)
    }

    pub fn decode_allowance(&self, bytes: &Bytes) -> Result<U256, anyhow::Error> {
        let allowance = ERC20::allowanceCall::abi_decode_returns(&bytes, true)?;
        Ok(allowance._0)
    }


    async fn symbol<T, P, N>(address: Address, client: P) -> Result<String, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        // ! There are cases like the MKR token where the symbol and name are not available
        let contract = ERC20::new(address, client.clone());
        let symbol = match contract.symbol().call().await {
            Ok(s) => s._0,
            Err(_) => "Unknown".to_string(),
        };
        Ok(symbol)
    }

    async fn name<T, P, N>(address: Address, client: P) -> Result<String, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        // ! There are cases like the MKR token where the symbol and name are not available
        let contract = ERC20::new(address, client.clone());
        let name = match contract.name().call().await {
            Ok(n) => n._0,
            Err(_) => "Unknown".to_string(),
        };
        Ok(name)
    }

    async fn decimals<T, P, N>(address: Address, client: P) -> Result<u8, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        let contract = ERC20::new(address, client.clone());
        let d = contract.decimals().call().await?._0;
        Ok(d)
    }

    async fn total_supply<T, P, N>(address: Address, client: P) -> Result<U256, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        let contract = ERC20::new(address, client.clone());
        let t = contract.totalSupply().call().await?._0;
        Ok(t)
    }
}

impl Default for ERC20Token {
    fn default() -> Self {
        Self {
            chain_id: 1,
            name: "Wrapped Ether".to_string(),
            address: Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
            decimals: 18,
            symbol: "WETH".to_string(),
            total_supply: U256::ZERO,
            icon: None,
        }
    }
}