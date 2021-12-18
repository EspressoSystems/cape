//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract RootStore {
    uint256[] internal _roots;
    uint64 internal _writeHead;

    constructor(uint64 nRoots) {
        // Set up the circular buffer for handling the last N roots
        require(nRoots > 1, "A least 2 roots required");

        _roots = new uint256[](nRoots);
        _writeHead = 1; // The first root value is 0 when the tree is empty
    }

    function _addRoot(uint256 newRoot) internal {
        _roots[_writeHead] = newRoot;
        _writeHead = (_writeHead + 1) % uint64(_roots.length);
    }

    function _containsRoot(uint256 root) internal view returns (bool) {
        // TODO evaluate gas cost of this loop based search vs mapping-assisted search
        for (uint256 i = 0; i < _roots.length; i++) {
            if (_roots[i] == root) {
                return true;
            }
        }
        return false;
    }

    function _checkContainsRoot(uint256 root) internal view {
        require(_containsRoot(root), "Root not found");
    }
}
