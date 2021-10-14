//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "hardhat/console.sol";

contract NullifiersMerkleTree {
    bytes64 root;

    uint256 constant N = 512;

    constructor() {}

    struct bytes64 {
        bytes32 hi;
        bytes32 lo;
    }

    // TODO probably not very efficient
    function EMPTY_HASH() private pure returns (bytes64 memory) {
        return bytes64(0, 0);
    }

    // TODO probably not very efficient
    function EMPTY_SUBTREE() private pure returns (bytes64 memory) {
        return bytes64(0, 0);
    }

    // TODO export this function to some "utils" library ?
    function are_equal_bytes64(bytes64 memory x, bytes64 memory y)
        private
        pure
        returns (bool)
    {
        return (x.lo == y.lo) && (x.hi == y.hi);
    }

    //   function check(bytes64[] calldata proof, bytes32 elem)
    //        public
    //        view
    //        returns (bool)
    //    {
    //        if (proof.length == 0) {
    //            revert("Proof has length zero");
    //        }
    //
    //        bytes64 memory running_hash = proof[0]; // or -1?
    //
    //        bytes64 memory h = elem_hash(elem);
    //        // bool[] elem_bit_vec = to_bits(elem_hash); // TODO to_bits
    //
    //        // the path only goes until a terminal node is reached, so skip
    //        // part of the bit-vec
    //        // uint256 start_bit = elem_bit_vec.length - proof.length;
    //        uint256 start_bit = 256 - proof.length;
    //
    //        // for (uint256 i = start_bit; i < elem_bit_vec.length; i++) {
    //        for (uint256 i = start_bit; i < 256; i++) {
    //            console.log(i);
    //            bytes32 sib = proof[i - start_bit];
    //            // TODO all bits
    //            bool sib_is_left = (uint256(h.hi) >> i) % 2 == 1;
    //
    //            bytes32 l;
    //            bytes32 r;
    //
    //            if (sib_is_left) {
    //                l = sib;
    //                r = running_hash;
    //            } else {
    //                l = running_hash;
    //                r = sib;
    //            }
    //            running_hash = branch_hash(l, r);
    //        }
    //
    //        bytes64 memory terminal_node = proof[proof.length - 1];
    //
    //        if (isEqualToRoot(running_hash)) {
    //            if (isEmptySubtree(terminal_node)) {
    //                return false;
    //            } else if (isLeafNode(terminal_node)) {
    //                // TODO Need to have the value to compare it.
    //                // return terminal_node = elem;
    //                return true;
    //            } else {
    //                revert("Wrong type of terminal node");
    //            }
    //        } else {
    //            // console.log("Running Hash:");
    //            // console.logBytes32(running_hash);
    //            // console.log("root");
    //            // console.logBytes32(root);
    //            revert("Hash mismatch");
    //        }
    //}

    function isEqualToRoot(bytes64 memory running_hash)
        private
        pure
        returns (bool)
    {
        // different storage locations
        // TODO just to avoid the warning
        assert(are_equal_bytes64(running_hash, running_hash));
        return false;
    }

    function isEmptySubtree(bytes64 memory node) private pure returns (bool) {
        return are_equal_bytes64(node, EMPTY_SUBTREE());
    }

    function isLeafNode(bytes64 memory node) private pure returns (bool) {
        return !are_equal_bytes64(node, EMPTY_SUBTREE());
    }

    function elem_hash(bytes memory elem) public pure returns (bytes memory) {
        //assert(elem[0] == 0); // TODO just to avoid the warning
        // TODO h(canonical_serialize(nul)) where h is Blake2B personalized with “AAPSet Elem”
        //return elem;
        return elem;
    }

    //    function leaf_hash(bytes32 elem) public pure returns (bytes memory) {
    //        // TODO h(canonical_serialize(nul)) where h is Blake2B personalized with “AAPSet Leaf”
    //        return keccak256(abi.encodePacked(elem));
    //    }

    function branch_hash(bytes64 calldata left, bytes64 calldata right)
        public
        pure
        returns (bytes64 memory)
    {
        // h("l"||l||"r"||r) where h is Blake2B personalized with “AAPSet Branch”
        // return keccak256(abi.encodePacked("l", left, "r", right));

        assert(are_equal_bytes64(left, left)); // TODO Just to avoid compiler warnings
        assert(are_equal_bytes64(right, right)); // TODO Just to avoid compiler warnings

        return bytes64(0, 0);
    }
}
