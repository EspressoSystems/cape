//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../CAPE.sol";

contract TestCapeTypes is CAPE {
    function typeNullifier(uint256 nf) public pure returns (uint256) {
        return nf;
    }
}
