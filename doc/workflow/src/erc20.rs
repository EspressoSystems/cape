//! This module describes the interface of some ERC20 token contract.

use ethers::prelude::*;

pub struct Erc20Contract {}

#[allow(dead_code, unused_variables)]
impl Erc20Contract {
    /// instantiate the contract instance, assert/revert if failed.
    /// in Solidity, achieved via:
    /// ```solidity
    /// import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
    /// contract MyContract {
    ///   IERC20 private _erc20Token;
    ///   constructor (address _myErc20) {
    ///      _erc20Token = IERC20(_myErc20); // will revert if fail
    ///   }
    /// }
    /// ```
    pub fn at(address: Address) -> Self {
        Self {}
    }

    /// https://docs.openzeppelin.com/contracts/4.x/api/token/erc20#IERC20-balanceOf-address-
    pub fn balance_of(&self, account: Address) -> U256 {
        unimplemented!();
    }

    /// https://docs.openzeppelin.com/contracts/4.x/api/token/erc20#IERC20-transfer-address-uint256-
    pub fn approve(&mut self, spender: Address, amount: U256) -> bool {
        true
    }

    /// https://docs.openzeppelin.com/contracts/4.x/api/token/erc20#IERC20-transferFrom-address-address-uint256-
    pub fn transfer_from(&mut self, sender: Address, recipient: Address, amount: U256) -> bool {
        true
    }

    /// https://docs.openzeppelin.com/contracts/4.x/api/token/erc20#IERC20-transfer-address-uint256-
    pub fn transfer(&mut self, recipient: Address, amount: U256) -> bool {
        true
    }
}
