//SPDX-License-Identifier: MIT
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
