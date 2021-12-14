//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "../CAPE.sol";

contract TestCAPE is CAPE {
    function insertNullifier(uint256 nullifier) public {
        return _insertNullifier(nullifier);
    }
}
