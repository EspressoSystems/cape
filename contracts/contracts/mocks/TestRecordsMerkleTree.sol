//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "../RecordsMerkleTree.sol";
import "../libraries/RescueLib.sol";

contract TestRecordsMerkleTree is RecordsMerkleTree {
    constructor(uint8 height) RecordsMerkleTree(height) {}

    function testUpdateRecordsMerkleTree(uint256[] memory elements) public {
        _updateRecordsMerkleTree(elements);
    }

    function doNothing() public {}
}
