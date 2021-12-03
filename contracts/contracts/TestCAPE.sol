//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./CAPE.sol";

contract TestCAPE is CAPE {
    function _hasNullifierAlreadyBeenPublished(uint256 _nullifier)
        public
        returns (bool)
    {
        return hasNullifierAlreadyBeenPublished(_nullifier);
    }

    function _insertNullifier(uint256 _nullifier) public {
        return insertNullifier(_nullifier);
    }
}
