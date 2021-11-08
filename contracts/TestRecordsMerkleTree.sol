//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "./Rescue.sol";
import "./RecordsMerkleTree.sol";

contract TestRecordsMerkleTree is RecordsMerkleTree {
    function test_update_records_merkle_tree(
        uint256[] memory _frontier,
        uint256[] memory _elements
    ) public {
        update_records_merkle_tree(_frontier, _elements);
    }

    function test_set_root_and_num_leaves(uint256 _root, uint64 _num_leaves)
        public
    {
        root_value = _root;
        num_leaves = _num_leaves;
    }
}
