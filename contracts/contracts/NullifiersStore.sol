//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract NullifiersStore {
    mapping(bytes => bool) private nullifiers;
    bytes32 private nullifiersCommitment;

    constructor() {
        nullifiersCommitment = 0; // Initial value of the nullifiers commitment
    }

    // Check if a nullifier has already been inserted
    function hasNullifierAlreadyBeenPublished(bytes memory _nullifier)
        internal
        view
        returns (bool)
    {
        return !nullifiers[_nullifier];
    }

    // Insert a nullifier into the set of nullifiers.
    // Also updates the commitment to the set of nullifiers by computing:
    //   new_commitment = keccak256(current_commitment || nullifier)
    // This function does not throw if the nullifier is already in the set for gas efficiency purposes.
    function insertNullifier(bytes memory _nullifier) internal {
        nullifiers[_nullifier] = true;
        // Update the commitment to the set of nullifiers
        nullifiersCommitment = keccak256(
            abi.encodePacked(nullifiersCommitment, _nullifier)
        );
    }

    function getNullifierSetCommitment() public view returns (bytes32) {
        return nullifiersCommitment;
    }
}
