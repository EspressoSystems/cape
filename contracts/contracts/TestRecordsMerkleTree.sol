//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "./Rescue.sol";
import "./RecordsMerkleTree.sol";

contract TestRecordsMerkleTree is RecordsMerkleTree {
    constructor(uint8 _height) RecordsMerkleTree(_height) {}

    function testCheckFrontier(uint256[] memory flattenedFrontier)
        public
        returns (bool)
    {
        return checkFrontier(flattenedFrontier);
    }

    function hashFrontierAndStoreHash(
        uint256[] memory flattenedFrontier,
        uint64 uid
    ) public {
        frontierHashValue = hashFrontier(flattenedFrontier, uid);
    }

    function testUpdateRecordsMerkleTree(
        uint256[] memory _frontier,
        uint256[] memory _elements
    ) public {
        updateRecordsMerkleTree(_frontier, _elements);
    }

    function testSetRootAndNumLeaves(uint256 _root, uint64 _numLeaves) public {
        rootValue = _root;
        numLeaves = _numLeaves;
    }

    function testSetFrontierHashValue(bytes32 _frontierHashValue) public {
        frontierHashValue = _frontierHashValue;
    }

    function doNothing() public {}
}
