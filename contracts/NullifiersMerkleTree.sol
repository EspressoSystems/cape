//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "hardhat/console.sol";

contract NullifiersMerkleTree {
    bytes32 root;

    uint256 constant N = 256;

    // uint64[8] ZERO_HASH = [0, 0, 0, 0, 0, 0, 0, 0];
    // uint64[8] EMPTY_SUBTREE = [0, 0, 0, 0, 0, 0, 0, 0];
    bytes32 EMPTY_HASH = 0; // TODO is that right?

    struct TerminalNode {
        bool isEmptySubtree;
        uint256 height;
        bytes elem;
    }

    constructor() {}

    function terminalNodeValue(TerminalNode memory node)
        public
        view
        returns (bytes32)
    {
        if (node.isEmptySubtree) {
            return EMPTY_HASH;
        } else {
            return terminalNodeValueNonEmpty(node);
        }
    }

    function to_bool_array(bytes32 as_bytes)
        public
        view
        returns (bool[N] memory bitvec)
    {
        for (uint256 i = 0; i < N; i++) {
            uint256 byte_idx = i / 8;
            bytes1 b = as_bytes[byte_idx];
            uint8 shift = 7 - uint8(i % 8);
            bitvec[i] = uint8(b >> shift) % 2 == 1;
        }
    }

    function terminalNodeValueNonEmpty(TerminalNode memory node)
        public
        view
        returns (bytes32)
    {
        bytes32 element_hash = elem_hash(node.elem);
        bytes32 running_hash = leaf_hash(node.elem);

        bool[256] memory sibblings = to_bool_array(element_hash);

        for (uint256 i = 0; i < node.height; i++) {
            bool sib_is_left = sibblings[i];

            if (sib_is_left) {
                running_hash = branch_hash(EMPTY_HASH, running_hash);
            } else {
                running_hash = branch_hash(running_hash, EMPTY_HASH);
            }
        }
        return running_hash;
    }

    function check(
        bytes32[] memory path,
        TerminalNode memory terminal_node,
        bytes memory elem
    ) public view returns (bool) {
        if (path.length == 0) {
            revert("Path has length zero");
        }

        bytes32 element_hash = elem_hash(elem);
        bytes32 running_hash = terminalNodeValue(terminal_node);

        // the path only goes until a terminal node is reached, so skip
        // part of the bit-vec
        uint256 start_bit = N - path.length;

        for (uint256 i = start_bit; i < N; i++) {
            bytes32 sib = path[i - start_bit];

            uint256 outer_idx = i / 64; // TODO 64 ?
            uint256 inner_idx = i % 64;

            uint8 c = uint8(element_hash[outer_idx] >> inner_idx);
            bool sib_is_left = (c % 2) == 1;

            bytes32 left;
            bytes32 right;

            if (sib_is_left) {
                left = sib;
                right = running_hash;
            } else {
                left = running_hash;
                right = sib;
            }
            running_hash = branch_hash(left, right);
        }

        // TerminalNode memory terminal_node = path[path.length - 1]; // TODO do we need this?

        if (isEqualToRoot(running_hash)) {
            if (terminal_node.isEmptySubtree) {
                return false;
            } else {
                // TODO is comparing the hashes acceptable?
                return keccak256(terminal_node.elem) == keccak256(elem);
            }
        } else {
            // console.log("Running Hash:");
            // console.logBytes32(running_hash);
            // console.log("root");
            // console.logBytes32(root);
            revert("Hash mismatch");
        }
    }

    function isEqualToRoot(bytes32 running_hash) private view returns (bool) {
        return running_hash == root;
    }

    function elem_hash(bytes memory input) public view returns (bytes32) {
        bytes memory domain_sep = "AAPSet Elem";
        console.log("inputs:");
        console.logBytes(input);

        bytes32 res = keccak256(abi.encodePacked(domain_sep, input));

        return res;
    }

    function leaf_hash(bytes memory input) public pure returns (bytes32) {
        bytes memory domain_sep = "AAPSet Leaf";
        return keccak256(abi.encodePacked(domain_sep, input));
    }

    function branch_hash(bytes32 left, bytes32 right)
        public
        pure
        returns (bytes32)
    {
        bytes memory domain_sep = "AAPSet Branch";
        return keccak256(abi.encodePacked(domain_sep, "l", left, "r", right));
    }
}
