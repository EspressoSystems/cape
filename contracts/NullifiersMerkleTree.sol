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

    function terminalNodeValueNonEmpty(TerminalNode memory node)
        public
        view
        returns (bytes32)
    {
        bytes32 element_hash = elem_hash(node.elem);
        bytes32 running_hash = leaf_hash(node.elem);

        for (uint256 i = 0; i < node.height; i++) {
            uint256 limb_idx = i / 64;
            uint256 bit_idx = i % 64;
            // TODO uncomment
            // bool sib_is_left = ((element_hash[limb_idx] >> bit_idx) % 2) == 1;
            // TODO comment
            bool sib_is_left = true;

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
        uint256 start_bit = 256 - path.length;

        // for (uint256 i = start_bit; i < elem_bit_vec.length; i++) {
        for (uint256 i = start_bit; i < 256; i++) {
            bytes32 sib = path[i - start_bit];

            uint256 outer_idx = i / 64;
            uint256 inner_idx = i % 64;
            // TODO uncomment
            //bool sib_is_left = ((element_hash[outer_idx] >> inner_idx) % 2) ==
            //    1;
            // TODO comment
            bool sib_is_left = false;

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

    function arrayEqual(bytes32 a, bytes32 b) private pure returns (bool) {
        for (uint256 i = 0; i < 8; i++) {
            if (a[i] != b[i]) {
                return false;
            }
        }
        return true;
    }

    function isEqualToRoot(bytes32 running_hash) private view returns (bool) {
        return arrayEqual(running_hash, root);
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

    // abi.encodePacked with uint64 arrays end up padded
    //    function pack(uint64[8] memory left, uint64[8] memory right)
    //        public
    //        returns (bytes memory)
    //    {
    //        // bytes memory data = abi.encodePacked("l"); // TODO: re-enable
    //        bytes memory data = abi.encodePacked();
    //
    //        for (uint256 i = 0; i < left.length; i++) {
    //            data = abi.encodePacked(data, left[i]);
    //        }
    //
    //        // data = abi.encodePacked(data, "r"); // TODO: re-enable
    //        for (uint256 i = 0; i < right.length; i++) {
    //            data = abi.encodePacked(data, right[i]);
    //        }
    //
    //        return data;
    //    }
    //
    //    function test_pack_u64() public returns (bytes memory) {
    //        uint64 u = 2 ^ 64;
    //        return abi.encodePacked(u);
    //    }
    //
    //    function formatInput(bytes memory input)
    //        public
    //        returns (uint64[2] memory output)
    //    {
    //        return blake.formatInput(input);
    //    }
    //
    //    function formatOutput(uint64[8] memory input)
    //        public
    //        returns (bytes32[2] memory)
    //    {
    //        return blake.formatOutput(input);
    //    }
    //
    //    function sendDataOnly(uint64[8] memory input)
    //        public
    //        returns (uint64[8] memory)
    //    {
    //        return input;
    //    }
}
