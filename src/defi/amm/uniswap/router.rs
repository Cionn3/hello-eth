// ! WIP

use alloy_primitives::{Address, Bytes, bytes::BufMut, U256};
use alloy_sol_types::{sol, SolCall, SolValue};


use std::str::FromStr;

// Deployed contract address

/// Also on OP Base
const ETH : &str = "0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD";
const OPTIMISM : &str = "0xCb1355ff08Ab38bBCE60111F1bb2B784bE25D7e8";
const BSC : &str = "0x4Dae2f939ACf50408e13d58534Ff8c2776d45265";
const ARBITRUM : &str = "0x5E325eDA8064b456f4781070C0738d849c824258";


sol! {
    #[sol(rpc)]
    contract UniversalRouterContract {
        function execute(bytes calldata commands, bytes[] calldata inputs, uint256 deadline)
        external
        payable
        checkDeadline(deadline);
        function execute(bytes calldata commands, bytes[] calldata inputs) public payable;
    
}
}

/// Universal Router Command Inputs
#[allow(non_camel_case_types)]
pub enum Input {
    /// 0x00
    V3_SWAP_EXACT_IN(
        // Recipient
        Address,

        // Token in amount
        U256,

        // Minimum Received of token out
        U256,

        // Encoded Token Path
        Bytes,

        // Are funds coming through Permit2
        bool
    ),

    /// 0x08
    V2_SWAP_EXACT_IN(
        // Recipient
        Address,

        // Token in amount
        U256,

        // Minimum Received of token out
        U256,

        // Token Path
        Vec<Address>,

        // Are funds coming through Permit2
        bool
    ),
}

impl Input {
    pub fn swap_v3_exact_in(
        recipient: Address,
        token_in_amount: U256,
        min_received: U256,
        token_path: Vec<Address>,
        permit: bool
    ) -> Self {
        let encoded_path = encode_token_path(token_path);
        Self::V3_SWAP_EXACT_IN(recipient, token_in_amount, min_received, encoded_path, permit)
    }

    pub fn swap_v2_exact_in(
        recipient: Address,
        token_in_amount: U256,
        min_received: U256,
        path: Vec<Address>,
        permit: bool
    ) -> Self {
        Self::V2_SWAP_EXACT_IN(recipient, token_in_amount, min_received, path, permit)
    }

    /// ABI encode the command input
    pub fn encode(&self) -> Bytes {
        let mut data = Vec::new();
        match self {
            Self::V3_SWAP_EXACT_IN(recipient, token_in_amount, min_received, encoded_path, permit) => {
                data.extend_from_slice(&recipient.abi_encode());
                data.extend_from_slice(&token_in_amount.abi_encode());
                data.extend_from_slice(&min_received.abi_encode());
                data.extend_from_slice(&encoded_path);
                data.put_u8(*permit as u8);
            }
            Self::V2_SWAP_EXACT_IN(recipient, token_in_amount, min_received, path, permit) => {
                data.extend_from_slice(&recipient.abi_encode());
                data.extend_from_slice(&token_in_amount.abi_encode());
                data.extend_from_slice(&min_received.abi_encode());
                for address in path {
                    data.extend_from_slice(&address.abi_encode());
                }
                data.put_u8(*permit as u8);
            }
        }
        Bytes::from(data)
    }
}


pub fn encode_token_path(path: Vec<Address>) -> Bytes {
    let mut data = Vec::new();
    for address in path {
        let address: [u8; 20] = address.into(); 
        data.extend_from_slice(&address);
    }
    Bytes::from(data)
}



/// Represents the Uniswap Universal Router
pub struct UniversalRouter {
    pub chain_id: u64,
    pub address: Address,
}

impl UniversalRouter {
    pub fn new(chain_id: u64) -> Result<Self, anyhow::Error> {
        let address = match chain_id {
            1 => Address::from_str(ETH)?,
            10 => Address::from_str(OPTIMISM)?,
            56 => Address::from_str(BSC)?,
            8453 => Address::from_str(ETH)?,
            42161 => Address::from_str(ARBITRUM)?,
            _ => return Err(anyhow::anyhow!("Unsupported chain id: {}", chain_id))
        };
        Ok(Self {
            chain_id,
            address
        })
    }

    /// Encode the execute function
    pub fn encode_execute(
        &self,
        inputs: Vec<Input>,
    ) -> Bytes {
        let mut commands = Vec::new();

        for input in &inputs {
            match input {
                Input::V3_SWAP_EXACT_IN(..) => {
                    commands.push(0x00);
                }
                Input::V2_SWAP_EXACT_IN(..) => {
                    commands.push(0x08);
                }
            }
        }

        let contract = UniversalRouterContract::execute_1Call {
            commands: Bytes::from(commands),
            inputs: inputs.iter().map(|input| input.encode()).collect(),
        };

        Bytes::from(contract.abi_encode())
    }

}