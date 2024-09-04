// ! Commonly used addresses

use alloy_primitives::{address, Address};
use anyhow::anyhow;

pub fn weth(chain_id: u64) -> Result<Address, anyhow::Error> {
    match chain_id {
        1 => Ok(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
        10 => Ok(address!("4200000000000000000000000000000000000006")),
        8453 => Ok(address!("4200000000000000000000000000000000000006")),
        42161 => Ok(address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1")),
        _ => Err(anyhow!("Unsupported chain id: {}", chain_id)),
}
}

pub fn wbnb(chain_id: u64) -> Result<Address, anyhow::Error> {
    if chain_id != 56 {
        return Err(anyhow!("Wrong ChainId expected 56 but got {}", chain_id));
    }
    Ok(address!("bb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"))
}

pub fn usdc(chain_id: u64) -> Result<Address, anyhow::Error> {
    match chain_id {
        1 => Ok(address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")),
        // USDC.e (Bridged from Ethereum)
        10 => Ok(address!("7F5c764cBc14f9669B88837ca1490cCa17c31607")),
        56 => Ok(address!("8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d")),
        8453 => Ok(address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")),
        // Not Bridged
        42161 => Ok(address!("af88d065e77c8cC2239327C5EDb3A432268e5831")),
        _ => Err(anyhow!("Unsupported chain id: {}", chain_id)),
    }
}

pub fn usdt(chain_id: u64) -> Result<Address, anyhow::Error> {
    match chain_id {
        1 => Ok(address!("dAC17F958D2ee523a2206206994597C13D831ec7")),
        10 => Ok(address!("94b008aA00579c1307B0EF2c499aD98a8ce58e58")),
        56 => Ok(address!("55d398326f99059fF775485246999027B3197955")),
        8453 => Err(anyhow!("USDT is not available on chain id: {}", chain_id)),
        42161 => Ok(address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9")),
        _ => Err(anyhow!("Unsupported chain id: {}", chain_id)),
    }
}

pub fn dai(chain_id: u64) -> Result<Address, anyhow::Error> {
    match chain_id {
        1 => Ok(address!("6B175474E89094C44Da98b954EedeAC495271d0F")),
        10 => Ok(address!("DA10009cBd5D07dd0CeCc66161FC93D7c9000da1")),
        56 => Ok(address!("1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3")),
        8453 => Ok(address!("50c5725949A6F0c72E6C4a641F24049A917DB0Cb")),
        42161 => Ok(address!("DA10009cBd5D07dd0CeCc66161FC93D7c9000da1")),
        _ => Err(anyhow!("Unsupported chain id: {}", chain_id)),
    }
}