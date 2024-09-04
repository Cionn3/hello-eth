pub mod erc20;
pub mod native;

use self::{erc20::ERC20Token, native::NativeCurrency};
use serde::{Deserialize, Serialize};


/// Represents a Currency, this can be a [NativeCurrency] to its chain (eg ETH, BNB) or any [ERC20Token]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Currency {
    Native(NativeCurrency),
    ERC20(ERC20Token),
}

impl Currency {
    /// Create a new Currency from a [NativeCurrency]
    pub fn from_native(native: NativeCurrency) -> Self {
        Self::Native(native)
    }

    /// Create a new Currency from an [ERC20Token]
    pub fn from_erc20(erc20: ERC20Token) -> Self {
        Self::ERC20(erc20)
    }

    pub fn is_native(&self) -> bool {
        matches!(self, Self::Native(_))
    }

    /// Get the ERC20 inside
    pub fn erc20(&self) -> Option<&ERC20Token> {
        match self {
            Self::ERC20(erc20) => Some(erc20),
            _ => None,
        }
    }

    pub fn symbol(&self) -> &String {
        match self {
            Self::Native(native) => &native.symbol,
            Self::ERC20(erc20) => &erc20.symbol,
        }
    }

    pub fn name(&self) -> &String {
        match self {
            Self::Native(native) => &native.name,
            Self::ERC20(erc20) => &erc20.name,
        }
    }

    pub fn decimals(&self) -> &u8 {
        match self {
            Self::Native(native) => &native.decimals,
            Self::ERC20(erc20) => &erc20.decimals,
        }
    }
}