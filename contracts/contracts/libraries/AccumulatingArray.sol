//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// @title AccumulatingArray library
/// @dev There are no dynamic in memory arrays in solidity.
/// This library can be used as a replacement.

library AccumulatingArray {
    struct Data {
        uint256[] items;
        uint256 index;
    }

    /// @dev Create a new AccumulatingArray
    /// @param maxLength the maximum number of items that can be added
    function create(uint256 maxLength) internal pure returns (Data memory) {
        return Data(new uint256[](maxLength), 0);
    }

    /// @param items the items to accumulate
    /// @dev Will revert if items past maxLength are added.
    function add(Data memory self, uint256[] memory items) internal pure {
        for (uint256 i = 0; i < items.length; i++) {
            self.items[i + self.index] = items[i];
        }
        self.index += items.length;
    }

    /// @param item the item to accumulate.
    /// @dev Will revert if items past maxLength are added.
    function add(Data memory self, uint256 item) internal pure {
        self.items[self.index] = item;
        self.index += 1;
    }

    /// @return array with all the accumulated items
    function toArray(Data memory self) internal pure returns (uint256[] memory) {
        uint256[] memory out = new uint256[](self.index);
        for (uint256 i = 0; i < self.index; i++) {
            out[i] = self.items[i];
        }
        return out;
    }
}
