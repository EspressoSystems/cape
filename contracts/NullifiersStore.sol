//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract NullifiersStore {
    mapping(bytes => bool) private nullifiers;
    bytes32 private nullifiers_commitment;

    constructor() {
        nullifiers_commitment = 0; // Initial value of the nullifiers commitment
    }

    // Check if a nullifier has already been inserted
    function has_nullifier_already_been_published(bytes memory _nullifier)
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
    function insert_nullifier(bytes memory _nullifier) internal {
        nullifiers[_nullifier] = true;
        // Update the commitment to the set of nullifiers
        nullifiers_commitment = keccak256(
            abi.encodePacked(nullifiers_commitment, _nullifier)
        );
    }

    function get_nullifier_set_commitment() public view returns (bytes32) {
        return nullifiers_commitment;
    }
}
