// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

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
