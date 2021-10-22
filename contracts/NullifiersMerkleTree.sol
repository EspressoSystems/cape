//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "hardhat/console.sol";

contract NullifiersMerkleTree {
    bytes32 stored_root;
    uint256 constant N = 256;

    // TODO how to get these in ethers-rs?
    enum MembershipCheckResult {
        NOT_IN_SET,
        IN_SET,
        ROOT_MISMATCH
    }

    // uint64[8] ZERO_HASH = [0, 0, 0, 0, 0, 0, 0, 0];
    // uint64[8] EMPTY_SUBTREE = [0, 0, 0, 0, 0, 0, 0, 0];
    bytes32 EMPTY_HASH = 0; // TODO is that right?

    struct TerminalNode {
        bool isEmptySubtree;
        uint256 height;
        bytes elem;
    }

    constructor() {}

    function validate_and_apply(
        bytes32 new_root,
        bytes32[] memory path,
        TerminalNode memory terminal_node,
        bytes memory elem
    ) public {
        if (!validate(new_root, path, terminal_node, elem)) {
            revert("Proof invalid");
        }
        store_root(new_root);
    }

    function validate(
        bytes32 new_root,
        bytes32[] memory path,
        TerminalNode memory terminal_node,
        bytes memory elem
    ) public view returns (bool) {
        if (
            is_in_set(stored_root, path, terminal_node, elem) !=
            MembershipCheckResult.NOT_IN_SET
        ) {
            return false;
        }
        if (
            is_in_set(new_root, path, terminal_node, elem) !=
            MembershipCheckResult.IN_SET
        ) {
            return false;
        }
        return true;
    }

    function store_root(bytes32 new_root) internal {
        stored_root = new_root;
    }

    // This function is SetMerkleProof::check(&self, elem: Nullifier, root: &set_hash::Hash) -> Result<bool, set_hash::Hash>
    function is_in_set(
        bytes32 root,
        bytes32[] memory path,
        TerminalNode memory terminal_node,
        bytes memory elem
    ) public view returns (MembershipCheckResult) {
        bytes32 element_hash = elem_hash(elem);
        bytes32 running_hash = terminalNodeValue(terminal_node);

        // the path only goes until a terminal node is reached, so skip
        // part of the bit-vec
        uint256 start_bit = N - path.length;

        bool[256] memory sibblings = to_bool_array(element_hash);

        for (uint256 i = start_bit; i < N; i++) {
            bytes32 sib = path[i - start_bit];
            bool sib_is_left = sibblings[i];

            if (sib_is_left) {
                running_hash = branch_hash(sib, running_hash);
            } else {
                running_hash = branch_hash(running_hash, sib);
            }
        }

        // TerminalNode memory terminal_node = path[path.length - 1]; // TODO do we need this?

        if (running_hash == root) {
            if (terminal_node.isEmptySubtree) {
                return MembershipCheckResult.NOT_IN_SET;
            } else {
                // TODO is comparing the hashes acceptable?
                return
                    keccak256(terminal_node.elem) == keccak256(elem)
                        ? MembershipCheckResult.IN_SET
                        : MembershipCheckResult.NOT_IN_SET;
            }
        } else {
            return MembershipCheckResult.ROOT_MISMATCH;
        }
    }

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
        pure
        returns (bool[N] memory bitvec)
    {
        for (uint256 i = 0; i < N; i++) {
            uint256 byte_idx = i / 8;
            bytes1 b = as_bytes[byte_idx];
            uint8 shift = uint8(i % 8);
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
