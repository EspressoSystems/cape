// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "../libraries/RescueLib.sol";

contract TestRescue {
    function doNothing() public {}

    function hash(
        uint256 a,
        uint256 b,
        uint256 c
    ) public returns (uint256) {
        return RescueLib.hash(a, b, c);
    }
}
