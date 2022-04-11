// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "../libraries/RescueLib.sol";

contract TestRescue {
    function doNothing() public {}

    function hash(
        uint256 a,
        uint256 b,
        uint256 c
    ) public returns (uint256) {
        return RescueLib.hash(a, b, c);
    }

    function commit(uint256[15] memory inputs) public returns (uint256) {
        return RescueLib.commit(inputs);
    }
}
