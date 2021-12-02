//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract NullifiersStore {
    mapping(uint256 => bool) private nullifiers;
    bytes32 private nullifiersCommitment;

    constructor() {}

    // Check if a nullifier has already been inserted
    function hasNullifierAlreadyBeenPublished(uint256 _nullifier)
        public
        view
        returns (bool)
    {
        return !nullifiers[_nullifier];
    }

    // Insert a nullifier into the set of nullifiers.
    function insertNullifier(uint256 _nullifier) internal {
        nullifiers[_nullifier] = true;
    }
}
