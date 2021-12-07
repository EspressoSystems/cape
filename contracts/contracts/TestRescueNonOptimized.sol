//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "./RescueNonOptimized.sol";

contract TestRescueNonOptimized is RescueNonOptimized {
    function doNothing() public {}
}
