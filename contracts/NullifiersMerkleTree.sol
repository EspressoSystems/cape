//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "./BLAKE2b/BLAKE2b.sol";

contract NullifiersMerkleTree {
    uint64[8] root;

    uint256 constant N = 512;

    // uint64[8] ZERO_HASH = [0, 0, 0, 0, 0, 0, 0, 0];
    // uint64[8] EMPTY_SUBTREE = [0, 0, 0, 0, 0, 0, 0, 0];
    uint64[8] EMPTY_HASH = [0, 0, 0, 0, 0, 0, 0, 0];

    struct TerminalNode {
        bool isEmptySubtree;
        uint256 height;
        bytes elem;
    }

    constructor() {}

    function terminalNodeValue(TerminalNode memory node)
        public
        returns (uint64[8] memory)
    {
        if (node.isEmptySubtree) {
            return EMPTY_HASH;
        } else {
            return terminalNodeValueNonEmpty(node);
        }
    }

    function terminalNodeValueNonEmpty(TerminalNode memory node)
        public
        returns (uint64[8] memory)
    {
        uint64[8] memory element_hash = elem_hash(node.elem);
        uint64[8] memory running_hash = leaf_hash(node.elem);

        for (uint256 i = 0; i < node.height; i++) {
            uint256 limb_idx = i / 64;
            uint256 bit_idx = i % 64;
            bool sib_is_left = ((element_hash[limb_idx] >> bit_idx) % 2) == 1;

            if (sib_is_left) {
                running_hash = branch_hash(EMPTY_HASH, running_hash);
            } else {
                running_hash = branch_hash(running_hash, EMPTY_HASH);
            }
        }
        return running_hash;
    }

    // TODO: could the blake2 contract function be view or pure?
    function check(
        uint64[8][] memory path,
        TerminalNode memory terminal_node,
        bytes memory elem
    ) public returns (bool) {
        if (path.length == 0) {
            revert("Path has length zero");
        }

        uint64[8] memory element_hash = elem_hash(elem);
        uint64[8] memory running_hash = terminalNodeValue(terminal_node);

        // the path only goes until a terminal node is reached, so skip
        // part of the bit-vec
        uint256 start_bit = 256 - path.length;

        // for (uint256 i = start_bit; i < elem_bit_vec.length; i++) {
        for (uint256 i = start_bit; i < 256; i++) {
            uint64[8] memory sib = path[i - start_bit];

            uint256 outer_idx = i / 64;
            uint256 inner_idx = i % 64;
            bool sib_is_left = ((element_hash[outer_idx] >> inner_idx) % 2) ==
                1;

            uint64[8] memory left;
            uint64[8] memory right;

            if (sib_is_left) {
                left = sib;
                right = running_hash;
            } else {
                left = running_hash;
                right = sib;
            }
            running_hash = branch_hash(left, right);
        }

        // uint64[8] memory terminal_node = path[path.length - 1];

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

    function arrayEqual(uint64[8] memory a, uint64[8] memory b)
        private
        pure
        returns (bool)
    {
        for (uint256 i = 0; i < 8; i++) {
            if (a[i] != b[i]) {
                return false;
            }
        }
        return true;
    }

    function isEqualToRoot(uint64[8] memory running_hash)
        private
        view
        returns (bool)
    {
        return arrayEqual(running_hash, root);
    }

    function elem_hash(bytes memory input) public returns (uint64[8] memory) {
        BLAKE2b blake = new BLAKE2b();
        bytes memory persona = "AAPSet Elem";
        console.log("inputs:");
        console.logBytes(input);

        uint64[8] memory res_u64 = blake.blake2b_full(
            input,
            "",
            "",
            persona,
            64
        );

        return res_u64;
    }

    function leaf_hash(bytes memory input) public returns (uint64[8] memory) {
        BLAKE2b blake = new BLAKE2b();
        return blake.blake2b_full(input, "", "", "AAPSet Leaf", 64);
    }

    function branch_hash(uint64[8] memory left, uint64[8] memory right)
        public
        returns (uint64[8] memory)
    {
        BLAKE2b blake = new BLAKE2b();
        bytes memory persona = "AAPSet Branch";
        return blake.blake2b_full(pack(left, right), "", "", persona, 64);
    }

    //    function leaf_hash(bytes32 elem) public pure returns (bytes memory) {
    //        // TODO h(canonical_serialize(nul)) where h is Blake2B personalized with “AAPSet Leaf”
    //        return keccak256(abi.encodePacked(elem));
    //    }

    function branch_hash_with_updates(bytes memory left, bytes memory right)
        public
        returns (uint64[8] memory)
    {
        // h("l"||l||"r"||r) where h is Blake2B personalized with “AAPSet Branch”

        BLAKE2b blake = new BLAKE2b();
        console.log("left input");
        console.logBytes(left);

        console.log("right input");
        console.logBytes(right);

        uint64[8] memory res_u64 = blake.blake2b_with_updates_branch(
            "AAPSet Branch",
            left,
            right
        );

        return res_u64;
    }

    // abi.encodePacked with uint64 arrays end up padded
    function pack(uint64[8] memory left, uint64[8] memory right)
        public
        returns (bytes memory)
    {
        // bytes memory data = abi.encodePacked("l"); // TODO: re-enable
        bytes memory data = abi.encodePacked();

        for (uint256 i = 0; i < left.length; i++) {
            data = abi.encodePacked(data, left[i]);
        }

        // data = abi.encodePacked(data, "r"); // TODO: re-enable
        for (uint256 i = 0; i < right.length; i++) {
            data = abi.encodePacked(data, right[i]);
        }

        return data;
    }

    function test_pack_u64() public returns (bytes memory) {
        uint64 u = 2 ^ 64;
        return abi.encodePacked(u);
    }
}
