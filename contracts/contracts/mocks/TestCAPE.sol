//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "../CAPE.sol";

contract TestCAPE is CAPE {
    constructor(uint8 height) CAPE(height) {}

    function insertNullifier(uint256 nullifier) public {
        return _insertNullifier(nullifier);
    }

    function checkBurn(bytes memory extraProofBoundData) public {
        return _checkBurn(extraProofBoundData);
    }

    function hasBurnPrefix(bytes memory extraProofBoundData)
        public
        returns (bool)
    {
        return _hasBurnPrefix(extraProofBoundData);
    }

    function hasBurnDestination(bytes memory extraProofBoundData)
        public
        returns (bool)
    {
        return _hasBurnDestination(extraProofBoundData);
    }
}
