//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./AAPE.sol";

contract TestAAPE is AAPE {
    function _has_nullifier_already_been_published(bytes memory _nullifier)
        public
        returns (bool)
    {
        return has_nullifier_already_been_published(_nullifier);
    }

    function _insert_nullifier(bytes memory _nullifier) public {
        return insert_nullifier(_nullifier);
    }
}
