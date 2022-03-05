// SPDX-License-Identifier: GPL-3.0-or-later

// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import "../RootStore.sol";

contract TestRootStore is RootStore {
    constructor(uint64 nRoots) RootStore(nRoots) {}

    function addRoot(uint256 lastRoot) public {
        _addRoot(lastRoot);
    }

    function containsRoot(uint256 root) public view returns (bool) {
        return _containsRoot(root);
    }

    function checkContainsRoot(uint256 root) public view {
        _checkContainsRoot(root);
    }
}
