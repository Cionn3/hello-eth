use alloy_primitives::utils::parse_units;
use alloy_primitives::{Address, U256};
use alloy_rpc_types::BlockId;

use alloy_contract::private::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;

use serde::{Deserialize, Serialize};

use crate::abi::uniswap::pool::v2;
use crate::defi::currency::erc20::ERC20Token;
use crate::defi::utils::chain_link::get_token_price;

use super::super::consts::*;
use crate::defi::utils::common_addr::*;

/// Represents a Uniswap V2 Pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniswapV2Pool {
    pub chain_id: u64,
    pub address: Address,
    pub token0: ERC20Token,
    pub token1: ERC20Token,
    #[serde(skip)]
    state: Option<State>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct State {
    pub reserve0: U256,
    pub reserve1: U256,
    pub block: u64,
}

impl UniswapV2Pool {
    pub fn new(chain_id: u64, address: Address, token0: ERC20Token, token1: ERC20Token) -> Self {
        // reorder tokens
        let (token0, token1) = if token0.address < token1.address {
            (token0, token1)
        } else {
            (token1, token0)
        };

        Self {
            chain_id,
            address,
            token0,
            token1,
            state: None,
        }
    }

    /// Switch token0 and token1
    pub fn toggle_pair(&mut self) {
        std::mem::swap(&mut self.token0, &mut self.token1);
    }

    /// Return a reference to the state of this pool
    pub fn state(&self) -> Option<&State> {
        self.state.as_ref()
    }

    /// Update the state for this pool
    pub fn update_state(&mut self, state: State) {
        self.state = Some(state);
    }

    /// Fetch the state of the pool at a given block
    /// If block is None, the latest block is used
    pub async fn fetch_state<T, P, N>(
        client: P,
        pool: Address,
        block: Option<BlockId>,
    ) -> Result<State, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        let reserves = v2::get_reserves(pool, client, block).await?;
        let reserve0 = U256::from(reserves.0);
        let reserve1 = U256::from(reserves.1);

        Ok(State {
            reserve0,
            reserve1,
            block: reserves.2 as u64,
        })
    }

    pub fn simulate_swap(&self, token_in: Address, amount_in: U256) -> Result<U256, anyhow::Error> {
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("State not initialized"))?;

        if self.token0.address == token_in {
            Ok(self.get_amount_out(amount_in, state.reserve0, state.reserve1))
        } else {
            Ok(self.get_amount_out(amount_in, state.reserve1, state.reserve0))
        }
    }

    pub fn simulate_swap_mut(
        &mut self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, anyhow::Error> {
        let mut state = self
            .state
            .clone()
            .ok_or_else(|| anyhow::anyhow!("State not initialized"))?;

        if self.token0.address == token_in {
            let amount_out = self.get_amount_out(amount_in, state.reserve0, state.reserve1);

            state.reserve0 += amount_in;
            state.reserve1 -= amount_out;
            self.state = Some(state);

            Ok(amount_out)
        } else {
            let amount_out = self.get_amount_out(amount_in, state.reserve1, state.reserve0);

            state.reserve0 -= amount_out;
            state.reserve1 += amount_in;
            self.state = Some(state);

            Ok(amount_out)
        }
    }

    /// Calculates the amount received for a given `amount_in` `reserve_in` and `reserve_out`.
    pub fn get_amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }
        let fee = (10000 - (300 / 10)) / 10; //Fee of 300 => (10,000 - 30) / 10  = 997
        let amount_in_with_fee = amount_in * U256::from(fee);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;

        numerator / denominator
    }

    /// Calculates the price of the base token in terms of the quote token.
    ///
    /// Returned as a Q64 fixed point number.
    pub fn calculate_price_64_x_64(&self, base_token: Address) -> Result<u128, anyhow::Error> {
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("State not initialized"))?;

        let decimal_shift = self.token0.decimals as i8 - self.token1.decimals as i8;

        let (r_0, r_1) = if decimal_shift < 0 {
            (
                U256::from(state.reserve0)
                    * U256::from(10u128.pow(decimal_shift.unsigned_abs() as u32)),
                U256::from(state.reserve1),
            )
        } else {
            (
                U256::from(state.reserve0),
                U256::from(state.reserve1) * U256::from(10u128.pow(decimal_shift as u32)),
            )
        };

        if base_token == self.token0.address {
            if r_0.is_zero() {
                Ok(U128_0X10000000000000000)
            } else {
                div_uu(r_1, r_0)
            }
        } else if r_1.is_zero() {
            Ok(U128_0X10000000000000000)
        } else {
            div_uu(r_0, r_1)
        }
    }

    /// Get the usd value of token0 and token1 at a given block
    /// If block is None, the latest block is used
    pub async fn tokens_usd<T, P, N>(
        &self,
        client: P,
        block: Option<BlockId>,
    ) -> Result<(f64, f64), anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        // find a known token that we can get its usd value
        let mut token0_usd = get_token_price(
            client.clone(),
            block.clone(),
            self.chain_id,
            self.token0.address,
        )
        .await?;
        let mut token1_usd =
            get_token_price(client, block, self.chain_id, self.token1.address).await?;

        // case 1 token0 is unknown
        if token0_usd == 0.0 && token1_usd != 0.0 {
            let unit = parse_units("1", self.token0.decimals)?.get_absolute();
            let p_in_token1 = self.simulate_swap(self.token0.address, unit)?;
            let p_in_token1 = p_in_token1.to_string().parse::<f64>()?;
            let p_in_usd = p_in_token1 * token1_usd;
            token0_usd = p_in_usd;
        }

        // case 2 token1 is unknown
        if token1_usd == 0.0 && token0_usd != 0.0 {
            let unit = parse_units("1", self.token1.decimals)?.get_absolute();
            let p_in_token0 = self.simulate_swap(self.token1.address, unit)?;
            let p_in_token0 = p_in_token0.to_string().parse::<f64>()?;
            let p_in_usd = p_in_token0 * token0_usd;
            token1_usd = p_in_usd;
        }

        Ok((token0_usd, token1_usd))
    }

    /// Does pair support getting values in usd
    ///
    /// We check if at least one of the tokens is a stable coin or WETH
    pub fn supports_usd(&self) -> Result<bool, anyhow::Error> {
        let b = self.token0.address == weth(self.chain_id)?
            || self.token1.address == weth(self.chain_id)?
            || self.token0.address == usdc(self.chain_id)?
            || self.token1.address == usdc(self.chain_id)?
            || self.token0.address == usdt(self.chain_id)?
            || self.token1.address == usdt(self.chain_id)?
            || self.token0.address == dai(self.chain_id)?
            || self.token1.address == dai(self.chain_id)?;

        Ok(b)
    }
}

pub fn div_uu(x: U256, y: U256) -> Result<u128, anyhow::Error> {
    if !y.is_zero() {
        let mut answer;

        if x <= U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            answer = (x << U256_64) / y;
        } else {
            let mut msb = U256_192;
            let mut xc = x >> U256_192;

            if xc >= U256_0X100000000 {
                xc >>= U256_32;
                msb += U256_32;
            }

            if xc >= U256_0X10000 {
                xc >>= U256_16;
                msb += U256_16;
            }

            if xc >= U256_0X100 {
                xc >>= U256_8;
                msb += U256_8;
            }

            if xc >= U256_16 {
                xc >>= U256_4;
                msb += U256_4;
            }

            if xc >= U256_4 {
                xc >>= U256_2;
                msb += U256_2;
            }

            if xc >= U256_2 {
                msb += U256_1;
            }

            answer = (x << (U256_255 - msb)) / (((y - U256_1) >> (msb - U256_191)) + U256_1);
        }

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Ok(0);
        }

        let hi = answer * (y >> U256_128);
        let mut lo = answer * (y & U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);

        let mut xh = x >> U256_192;
        let mut xl = x << U256_64;

        if xl < lo {
            xh -= U256_1;
        }

        xl = xl.overflowing_sub(lo).0;
        lo = hi << U256_128;

        if xl < lo {
            xh -= U256_1;
        }

        xl = xl.overflowing_sub(lo).0;

        if xh != hi >> U256_128 {
            return Err(anyhow::anyhow!("Rounding Error"));
        }

        answer += xl / y;

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Ok(0_u128);
        }

        Ok(answer.to::<u128>())
    } else {
        Err(anyhow::anyhow!("Y is zero"))
    }
}
