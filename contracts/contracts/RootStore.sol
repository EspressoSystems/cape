// SPDX-License-Identifier: GPL-3.0-or-later

// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

contract RootStore {
    uint256[] internal _roots;
    mapping(uint256 => bool) internal _rootsMap;
    uint64 internal _writeHead;

    /// @dev Create a root store.
    /// @param nRoots The maximum number of roots to store
    constructor(uint64 nRoots) {
        // Set up the circular buffer for handling the last N roots
        require(nRoots > 1, "A least 2 roots required");

        _roots = new uint256[](nRoots);

        // Intially all roots are set to zero.
        // This value is such that no adversary can extend a branch from this root node.
        // See proposition 2, page 48 of the AT-Spec document EspressoSystems/AT-spec@01f71ce
    }

    /// @dev Add a root value. Only keep the latest nRoots ones.
    /// @param newRoot The value of the new root
    function _addRoot(uint256 newRoot) internal {
        require(!_rootsMap[newRoot], "Root already exists");

        // Ensure the root we will "overwrite" is removed.
        _rootsMap[_roots[_writeHead]] = false;

        _roots[_writeHead] = newRoot;
        _rootsMap[newRoot] = true;

        _writeHead = (_writeHead + 1) % uint64(_roots.length);
    }

    /// @dev Is the root value contained in the store?
    /// @param root The root value to find
    /// @return _ True if the root value is in the store, false otherwise
    function _containsRoot(uint256 root) internal view returns (bool) {
        return _rootsMap[root];
    }

    /// @dev Raise an exception if the root is not present in the store.
    /// @param root The required root value
    function _checkContainsRoot(uint256 root) internal view {
        require(_containsRoot(root), "Root not found");
    }
}
