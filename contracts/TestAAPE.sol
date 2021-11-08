//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./AAPE.sol";

contract TestAAPE is AAPE {
    function _hasNullifierAlreadyBeenPublished(bytes memory _nullifier)
        public
        returns (bool)
    {
        return hasNullifierAlreadyBeenPublished(_nullifier);
    }

    function _insertNullifier(bytes memory _nullifier) public {
        return insertNullifier(_nullifier);
    }
}
