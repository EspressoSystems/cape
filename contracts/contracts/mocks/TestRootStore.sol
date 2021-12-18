//SPDX-License-Identifier: MIT
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
