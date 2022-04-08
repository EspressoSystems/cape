// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

/// @title AccumulatingArray library
/// @dev This library simplifies inserting elements into an array by keeping track
///      of the insertion index.

library AccumulatingArray {
    struct Data {
        uint256[] items;
        uint256 index;
    }

    /// @dev Create a new AccumulatingArray
    /// @param length the number of items that will be inserted
    function create(uint256 length) internal pure returns (Data memory) {
        return Data(new uint256[](length), 0);
    }

    /// @param items the items to accumulate
    /// @dev Will revert if items past length are added.
    function add(Data memory self, uint256[] memory items) internal pure {
        for (uint256 i = 0; i < items.length; i++) {
            self.items[i + self.index] = items[i];
        }
        self.index += items.length;
    }

    /// @param item the item to accumulate.
    /// @dev Will revert if items past length are added.
    function add(Data memory self, uint256 item) internal pure {
        self.items[self.index] = item;
        self.index += 1;
    }

    function isEmpty(Data memory self) internal pure returns (bool) {
        return (self.index == 0);
    }
}
