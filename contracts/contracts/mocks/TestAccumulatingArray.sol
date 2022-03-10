// SPDX-License-Identifier: GPL-3.0-or-later

// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import {AccumulatingArray} from "../libraries/AccumulatingArray.sol";

contract TestAccumulatingArray {
    using AccumulatingArray for AccumulatingArray.Data;

    function accumulate(uint256[][] memory arrays, uint256 length)
        public
        pure
        returns (uint256[] memory)
    {
        AccumulatingArray.Data memory accumulated = AccumulatingArray.create(length);
        for (uint256 i = 0; i < arrays.length; i++) {
            accumulated.add(arrays[i]);
        }
        return accumulated.items;
    }

    // Adds single element arrays as individual items
    function accumulateWithIndividuals(uint256[][] memory arrays, uint256 length)
        public
        pure
        returns (uint256[] memory)
    {
        AccumulatingArray.Data memory accumulated = AccumulatingArray.create(length);
        for (uint256 i = 0; i < arrays.length; i++) {
            if (arrays[i].length == 1) {
                accumulated.add(arrays[i][0]);
            } else {
                accumulated.add(arrays[i]);
            }
        }
        return accumulated.items;
    }
}
