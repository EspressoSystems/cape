//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract RootStore {
    uint256[] internal _roots;
    mapping(uint256 => bool) internal _rootsMap;
    uint64 internal _writeHead;
    uint256 internal constant _EMPTY_NODE_VALUE = 0;

    /// @dev Create a root store.
    /// @param nRoots The maximum number of roots to store
    constructor(uint64 nRoots) {
        // Set up the circular buffer for handling the last N roots
        require(nRoots > 1, "A least 2 roots required");

        _roots = new uint256[](nRoots);

        // Set all roots to EMPTY_NODE_VALUE.
        // This value is such that no adversary can extend a branch from this root node.
        // See proposition 2, page 48 of the AT-Spec document SpectrumXYZ/AT-spec@01f71ce

        for (uint256 i = 0; i < nRoots; i++) {
            _roots[i] = _EMPTY_NODE_VALUE;
        }

        _writeHead = 1; // The first root value is 0 when the tree is empty
    }

    /// @dev Add a root value. Only keep the latest nRoots ones.
    /// @param newRoot The value of the new root
    function _addRoot(uint256 newRoot) internal {
        // Remove the root we will "overwrite"
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
