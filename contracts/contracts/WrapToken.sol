// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

// Learn more about the ERC20 implementation
// on OpenZeppelin docs: https://docs.openzeppelin.com/contracts/4.x/erc20
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @notice This token is only intended to be used for testing.
contract WrapToken is ERC20 {
    /// @notice The caller of this method receives 1000*10**6 units.
    constructor(string memory name, string memory symbol) ERC20(name, symbol) {
        _mint(msg.sender, 1000 * 10**6);
    }

    /// @notice Allows minting tokens by sending Ether to it.
    receive() external payable {
        _mint(msg.sender, 10**6 * msg.value);
    }

    function decimals() public view virtual override returns (uint8) {
        return 6;
    }

    function withdraw() external payable {
        uint256 balance = balanceOf(msg.sender);
        address payable sender = payable(msg.sender);
        _burn(sender, balance);
        sender.transfer(balance / 10**6);
    }
}
