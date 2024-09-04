pub mod fee_math;
pub mod lp_provider;

use alloy_primitives::{Address, I256, U256, utils::format_units};
use alloy_rpc_types::{BlockId, Log};

use alloy_contract::private::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::try_join;
use uniswap_v3_math::{tick_bitmap::position, tick_math::*};

use super::super::consts::*;
use crate::defi::utils::common_addr::*;
use crate::defi::utils::chain_link::get_token_price;
use crate::utils::logs::events::SwapData;
use crate::{
    abi::uniswap::pool::v3::{self, *},
    defi::currency::erc20::ERC20Token,
};

/// Represents a Uniswap V3 Pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniswapV3Pool {
    pub chain_id: u64,
    pub address: Address,
    pub fee: u32,
    pub token0: ERC20Token,
    pub token1: ERC20Token,
    #[serde(skip)]
    state: Option<State>,
}

/// Represents the volume of a pool that occured at some point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolVolume {
    pub buy_volume: U256,
    pub sell_volume: U256,
    pub swaps: Vec<SwapData>,
}

impl PoolVolume {
    pub fn buy_volume_usd(&self, usd_value: f64, decimals: u8) -> Result<f64, anyhow::Error> {
        let formatted = format_units(self.buy_volume, decimals)?.parse::<f64>()?;
        Ok(formatted * usd_value)
    }

    pub fn sell_volume_usd(&self, usd_value: f64, decimals: u8) -> Result<f64, anyhow::Error> {
        let formatted = format_units(self.sell_volume, decimals)?.parse::<f64>()?;
        Ok(formatted * usd_value)
}
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct State {
    pub liquidity: u128,
    pub sqrt_price: U256,
    pub tick: i32,
    pub tick_spacing: i32,
    pub tick_bitmap: HashMap<i16, U256>,
    pub ticks: HashMap<i32, TickInfo>,
    pub pool_tick: PoolTick,
}


#[allow(dead_code)]
struct CurrentState {
    amount_specified_remaining: I256,
    amount_calculated: I256,
    sqrt_price_x_96: U256,
    tick: i32,
    liquidity: u128,
}

#[derive(Default)]
#[allow(dead_code)]
struct StepComputations {
    pub sqrt_price_start_x_96: U256,
    pub tick_next: i32,
    pub initialized: bool,
    pub sqrt_price_next_x96: U256,
    pub amount_in: U256,
    pub amount_out: U256,
    pub fee_amount: U256,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TickInfo {
    liquidity_gross: u128,
    liquidity_net: i128,
    initialized: bool,
}

#[derive(Debug, Clone)]
pub struct PoolTick {
    pub tick: i32,
    pub liquidity_net: i128,
    pub block: u64,
}

impl UniswapV3Pool {
    /// Create a new Uniswap V3 Pool
    ///
    /// Tokens are ordered by address
    pub fn new(
        chain_id: u64,
        address: Address,
        fee: u32,
        token0: ERC20Token,
        token1: ERC20Token,
    ) -> Self {
        // reorder tokens
        let (token0, token1) = if token0.address < token1.address {
            (token0, token1)
        } else {
            (token1, token0)
        };

        Self {
            chain_id,
            address,
            fee,
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
        pool: Address,
        client: P,
        block: Option<BlockId>,
    ) -> Result<State, anyhow::Error>
    where
        T: Transport + Clone,
        P: Provider<T, N> + Clone,
        N: Network,
    {
        let (sqrt_price, tick, _, _, _, _, _) =
            v3::slot0(pool, client.clone(), block.clone()).await?;
        let (word_position, _) = position(tick);

        let liquidity = v3::liquidity(pool, client.clone(), block.clone());
        let tick_spacing = v3::tick_spacing(pool, client.clone());
        let tick_bitmap = v3::tick_bitmap(pool, word_position, client.clone(), block.clone());
        let ticks = v3::ticks(pool, tick, client.clone(), block.clone());

        let (liquidity, tick_spacing, tick_bitmap, ticks) =
            try_join!(liquidity, tick_spacing, tick_bitmap, ticks)?;

        let mut tick_bitmap_map = HashMap::new();
        tick_bitmap_map.insert(word_position, tick_bitmap);

        let liquidity_gross = ticks.0;
        let liquidity_net = ticks.1;
        let initialized = ticks.7;

        let ticks_info = TickInfo {
            liquidity_gross,
            liquidity_net,
            initialized,
        };

        let block = if let Some(b) = block {
            b.as_u64().unwrap_or(0)
        } else {
            0
        };
        let pool_tick = PoolTick {
            tick,
            liquidity_net,
            block,
        };

        let mut ticks_map = HashMap::new();
        ticks_map.insert(tick, ticks_info);

        Ok(State {
            liquidity,
            sqrt_price,
            tick,
            tick_spacing,
            tick_bitmap: tick_bitmap_map,
            ticks: ticks_map,
            pool_tick,
        })
    }

    pub fn simulate_swap(&self, token_in: Address, amount_in: U256) -> Result<U256, anyhow::Error> {
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("State not initialized"))?;

        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        let zero_for_one = token_in == self.token0.address;

        // Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one {
            MIN_SQRT_RATIO + U256_1
        } else {
            MAX_SQRT_RATIO - U256_1
        };

        // Initialize a mutable state state struct to hold the dynamic simulated state of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: state.sqrt_price.clone(), //Active price on the pool
            amount_calculated: I256::ZERO, //Amount of token_out that has been calculated
            amount_specified_remaining: I256::from_raw(amount_in), //Amount of token_in that has not been swapped
            tick: state.tick.clone(),                              //Current i24 tick of the pool
            liquidity: state.liquidity.clone(), //Current available liquidity in the tick range
        };

        while current_state.amount_specified_remaining != I256::ZERO
            && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96
        {
            // Initialize a new step struct to hold the dynamic state of the pool at each step
            let mut step = StepComputations {
                // Set the sqrt_price_start_x_96 to the current sqrt_price_x_96
                sqrt_price_start_x_96: current_state.sqrt_price_x_96,
                ..Default::default()
            };

            // Get the next tick from the current tick
            (step.tick_next, step.initialized) =
                uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                    &state.tick_bitmap,
                    current_state.tick,
                    state.tick_spacing,
                    zero_for_one,
                )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            // Target spot price
            let swap_target_sqrt_ratio = if zero_for_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            // Compute swap step and update the current state
            (
                current_state.sqrt_price_x_96,
                step.amount_in,
                step.amount_out,
                step.fee_amount,
            ) = uniswap_v3_math::swap_math::compute_swap_step(
                current_state.sqrt_price_x_96,
                swap_target_sqrt_ratio,
                current_state.liquidity,
                current_state.amount_specified_remaining,
                self.fee,
            )?;

            // Decrement the amount remaining to be swapped and amount received from the step
            current_state.amount_specified_remaining = current_state
                .amount_specified_remaining
                .overflowing_sub(I256::from_raw(
                    step.amount_in.overflowing_add(step.fee_amount).0,
                ))
                .0;

            current_state.amount_calculated -= I256::from_raw(step.amount_out);

            // If the price moved all the way to the next price, recompute the liquidity change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net = if let Some(info) = state.ticks.get(&step.tick_next) {
                        info.liquidity_net
                    } else {
                        0
                    };

                    // we are on a tick boundary, and the next tick is initialized, so we must charge a protocol fee
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Err(anyhow::anyhow!("Liquidity underflow"));
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                // Increment the current tick
                current_state.tick = if zero_for_one {
                    step.tick_next.wrapping_sub(1)
                } else {
                    step.tick_next
                }
                // If the current_state sqrt price is not equal to the step sqrt price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(
                    current_state.sqrt_price_x_96,
                )?;
            }
        }

        let amount_out = (-current_state.amount_calculated).into_raw();

        Ok(amount_out)
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

        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        let zero_for_one = token_in == self.token0.address;

        // Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one {
            MIN_SQRT_RATIO + U256_1
        } else {
            MAX_SQRT_RATIO - U256_1
        };

        // Initialize a mutable state state struct to hold the dynamic simulated state of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: state.sqrt_price, //Active price on the pool
            amount_calculated: I256::ZERO,     //Amount of token_out that has been calculated
            amount_specified_remaining: I256::from_raw(amount_in), //Amount of token_in that has not been swapped
            tick: state.tick,                                      //Current i24 tick of the pool
            liquidity: state.liquidity, //Current available liquidity in the tick range
        };

        while current_state.amount_specified_remaining != I256::ZERO
            && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96
        {
            // Initialize a new step struct to hold the dynamic state of the pool at each step
            let mut step = StepComputations {
                // Set the sqrt_price_start_x_96 to the current sqrt_price_x_96
                sqrt_price_start_x_96: current_state.sqrt_price_x_96,
                ..Default::default()
            };

            // Get the next tick from the current tick
            (step.tick_next, step.initialized) =
                uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                    &state.tick_bitmap,
                    current_state.tick,
                    state.tick_spacing,
                    zero_for_one,
                )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            // Target spot price
            let swap_target_sqrt_ratio = if zero_for_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            // Compute swap step and update the current state
            (
                current_state.sqrt_price_x_96,
                step.amount_in,
                step.amount_out,
                step.fee_amount,
            ) = uniswap_v3_math::swap_math::compute_swap_step(
                current_state.sqrt_price_x_96,
                swap_target_sqrt_ratio,
                current_state.liquidity,
                current_state.amount_specified_remaining,
                self.fee,
            )?;

            // Decrement the amount remaining to be swapped and amount received from the step
            current_state.amount_specified_remaining = current_state
                .amount_specified_remaining
                .overflowing_sub(I256::from_raw(
                    step.amount_in.overflowing_add(step.fee_amount).0,
                ))
                .0;

            current_state.amount_calculated -= I256::from_raw(step.amount_out);

            // If the price moved all the way to the next price, recompute the liquidity change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net = if let Some(info) = state.ticks.get(&step.tick_next) {
                        info.liquidity_net
                    } else {
                        0
                    };

                    // we are on a tick boundary, and the next tick is initialized, so we must charge a protocol fee
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Err(anyhow::anyhow!("Liquidity underflow"));
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                // Increment the current tick
                current_state.tick = if zero_for_one {
                    step.tick_next.wrapping_sub(1)
                } else {
                    step.tick_next
                }
                // If the current_state sqrt price is not equal to the step sqrt price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(
                    current_state.sqrt_price_x_96,
                )?;
            }
        }

        // update the pool state
        state.liquidity = current_state.liquidity;
        state.sqrt_price = current_state.sqrt_price_x_96;
        state.tick = current_state.tick;

        self.state = Some(state);

        let amount_out = (-current_state.amount_calculated).into_raw();

        Ok(amount_out)
    }

    /// Calculate the price of token in terms of quote token
    pub fn calculate_price(&self, base_token: Address) -> Result<f64, anyhow::Error> {
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("State not initialized"))?;

        let tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(state.sqrt_price)?;
        let shift = self.token0.decimals as i8 - self.token1.decimals as i8;

        let price = match shift.cmp(&0) {
            Ordering::Less => 1.0001_f64.powi(tick) / 10_f64.powi(-shift as i32),
            Ordering::Greater => 1.0001_f64.powi(tick) * 10_f64.powi(shift as i32),
            Ordering::Equal => 1.0001_f64.powi(tick),
        };

        if base_token == self.token0.address {
            Ok(price)
        } else {
            Ok(1.0 / price)
        }
    }

    /// Get the usd values of token0 and token1 at a given block
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
        let mut token0_usd = get_token_price(client.clone(), block.clone(), self.chain_id, self.token0.address).await?;
        let mut token1_usd = get_token_price(client, block, self.chain_id, self.token1.address).await?;


        // case 1 token0 is unknown
        if token0_usd == 0.0 && token1_usd != 0.0 {
            let p_in_token1 = self.calculate_price(self.token0.address)?;
            let p_in_usd = p_in_token1 * token1_usd;
           token0_usd = p_in_usd;
        }

        // case 2 token1 is unknown
        if token1_usd == 0.0 && token0_usd != 0.0 {
            let p_in_token0 = self.calculate_price(self.token1.address)?;
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

    /// Get the volume of the pool
    pub fn get_volume_from_logs(&self, logs: Vec<Log>) -> Result<PoolVolume, anyhow::Error> {
        let mut buy_volume = U256::ZERO;
        let mut sell_volume = U256::ZERO;
        let mut swaps = Vec::new();

        for log in &logs {
            let swap_data = self.decode_swap(log)?;
            if swap_data.token_in.address == self.token1.address {
                buy_volume += swap_data.amount_in;
            }

            if swap_data.token_out.address == self.token0.address {
                sell_volume += swap_data.amount_out;
            }
            swaps.push(swap_data);
        }

        swaps.sort_by(|a, b| a.block.cmp(&b.block));

        Ok(PoolVolume {
            buy_volume,
            sell_volume,
            swaps,
        })
    }

    /// Decode a swap log against this pool
    pub fn decode_swap(&self, log: &Log) -> Result<SwapData, anyhow::Error> {
        let IUniswapV3Pool::Swap {
            amount0, amount1, ..
        } = log.log_decode()?.inner.data;

        let pair_address = log.address();
        let block = log.block_number;

        if pair_address != self.address {
            return Err(anyhow::anyhow!("Pool Address mismatch"));
        }

        let (amount_in, token_in) = if amount0.is_positive() {
            (amount0, self.token0.clone())
        } else {
            (amount1, self.token1.clone())
        };

        let (amount_out, token_out) = if amount1.is_negative() {
            (amount1, self.token1.clone())
        } else {
            (amount0, self.token0.clone())
        };

        if block.is_none() {
            // this should never happen
            return Err(anyhow::anyhow!("Block number is missing"));
        }

        let tx_hash = if let Some(hash) = log.transaction_hash {
            hash
        } else {
            return Err(anyhow::anyhow!("Transaction hash is missing"));
        };

        let amount_in = U256::from_str(&amount_in.to_string())?;
        // remove the - sign
        let amount_out = amount_out
            .to_string()
            .trim_start_matches('-')
            .parse::<U256>()?;

        Ok(SwapData {
            account: None,
            token_in,
            token_out,
            amount_in,
            amount_out,
            block: block.unwrap(),
            tx_hash: tx_hash.to_string(),
        })
    }
}