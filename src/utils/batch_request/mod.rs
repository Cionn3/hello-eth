use alloy_sol_types::sol;
use alloy_dyn_abi::DynSolType;
use alloy_primitives::{Address, U256};

use alloy_contract::private::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;


sol! {
    #[sol(rpc)]
    IGetErc20Balance,
    "src/utils/batch_request/abi/GetErc20Balance.json",
}

pub struct TokenBalance {
    pub token: Address,
    pub balance: U256,
}


pub async fn erc20_balance<T, P, N>(
    client: P,
    owner: Address,
    tokens: Vec<Address>,
) -> Result<Vec<TokenBalance>, anyhow::Error>
where
    T: Transport + Clone,
    P: Provider<T, N> + Clone,
    N: Network,
{
    let deployer = IGetErc20Balance::deploy_builder(client, tokens, owner);
    let res = deployer.call_raw().await?;

    let constructor_return = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
        DynSolType::Address,
        DynSolType::Uint(256),
    ])));

    let balances_return = constructor_return.abi_decode_sequence(&res)?;
    let mut balances = Vec::new();

    if let Some(balance_array) = balances_return.as_array() {
        for balance in balance_array {
            if let Some(balance_tuple) = balance.as_tuple() {
                let token = balance_tuple[0].as_address().unwrap();
                let balance = balance_tuple[1].as_uint().unwrap();
                balances.push(TokenBalance { token, balance: balance.0 });
            }
        }
    }

    Ok(balances)

}


#[cfg(test)]

mod tests {

    #[tokio::test]
    async fn test_erc20_balance() {
        use alloy_provider::{ProviderBuilder, WsConnect};
        use alloy_signer_local::PrivateKeySigner;
        use crate::prelude::{usdc, weth};
        use super::erc20_balance;

        let url = "wss://eth.merkle.io";
        let ws_connect = WsConnect::new(url);
        let client = ProviderBuilder::new().on_ws(ws_connect).await.unwrap();

        let weth = weth(1).unwrap();
        let usdc = usdc(1).unwrap();

        let owner = PrivateKeySigner::random();

        let tokens = vec![weth, usdc];

        let balances = erc20_balance(client, owner.address(), tokens).await.unwrap();

        assert_eq!(balances.len(), 2);

}

}