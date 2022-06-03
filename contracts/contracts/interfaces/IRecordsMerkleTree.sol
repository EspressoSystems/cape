// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

interface IRecordsMerkleTree {
    /// @param elements The list of elements to be appended to the current merkle tree described by the frontier.
    function updateRecordsMerkleTree(uint256[] memory elements) external;

    /// @notice Returns the root value of the Merkle tree.
    function getRootValue() external view returns (uint256);

    /// @notice Returns the height of the Merkle tree.
    function getHeight() external view returns (uint8);

    /// @notice Returns the number of leaves of the Merkle tree.
    function getNumLeaves() external view returns (uint64);
}
