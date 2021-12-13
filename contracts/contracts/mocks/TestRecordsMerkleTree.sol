//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "../Rescue.sol";
import "../RecordsMerkleTree.sol";

contract TestRecordsMerkleTree is RecordsMerkleTree {
    constructor(uint8 _height) RecordsMerkleTree(_height) {}

    function testUpdateRecordsMerkleTree(uint256[] memory _elements) public {
        updateRecordsMerkleTree(_elements);
    }

    function doNothing() public {}
}
