//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "hardhat/console.sol";

contract NullifiersMerkleTree {
    bytes32 stored_root;
    uint256 constant N = 170;

    // TODO how to get these in ethers-rs?
    enum MembershipCheckResult {
        NOT_IN_SET,
        IN_SET,
        ROOT_MISMATCH
    }

    // uint64[8] ZERO_HASH = [0, 0, 0, 0, 0, 0, 0, 0];
    // uint64[8] EMPTY_SUBTREE = [0, 0, 0, 0, 0, 0, 0, 0];
    bytes32 EMPTY_HASH = 0; // TODO is that right?
    uint256 EMPTY_NODE_ID = 0;
    uint256 ROOT_INDEX = 1;

    struct TerminalNode {
        bool isEmptySubtree;
        uint256 height;
        bytes elem;
    }

    ////////////////////////////////// NAIVE IMPLEMENTATION OF SPARSE MERKLE TREE
    struct TreeNode {
        bytes32 val;
        uint256 left;
        uint256 right;
        uint256 up; // In order to update the tree without recursion
        bool isTerminal;
    }
    mapping(uint256 => TreeNode) nodes;
    uint256 num_nodes;

    TreeNode EMPTY_NODE =
        TreeNode(EMPTY_HASH, EMPTY_NODE_ID, EMPTY_NODE_ID, EMPTY_NODE_ID, true);

    function is_terminal_node(uint256 node_id) internal returns (bool) {
        TreeNode memory left = nodes[nodes[node_id].left];
        TreeNode memory right = nodes[nodes[node_id].right];
        return left.isTerminal && right.isTerminal;
    }

    function is_leaf_node(TreeNode memory t) internal returns (bool) {
        return (t.left == EMPTY_NODE_ID && t.right == EMPTY_NODE_ID);
    }

    function insert(bytes memory elem) public returns (bool) {
        console.log("Inserting element:");
        console.logBytes(elem);

        bytes32 element_hash = elem_hash(elem);
        bool[N] memory siblings = to_bool_array(element_hash);

        // Move down the tree until finding an empty node
        uint256 i = 0;

        // Start at the root
        uint256 current_node_id = ROOT_INDEX;
        TreeNode memory current_node = nodes[current_node_id];

        while (i < N) {
            console.log("First loop");
            console.log("i=%s", i);
            console.log("current_node_id:%s", current_node_id);

            if (is_terminal_node(current_node_id)) {
                console.log("Node with id %s is terminal.", current_node_id);
                break;
            } else {
                console.log(
                    "Node with id %s is NOT terminal.",
                    current_node_id
                );
            }

            bool sib_is_left = siblings[i];

            if (sib_is_left) {
                current_node_id = current_node.right;
            } else {
                current_node_id = current_node.left;
            }
            current_node = nodes[current_node_id];
            i += 1;
        }

        // Start filling the tree until the with more empty nodes
        while (i < N) {
            console.log("Second loop");
            console.log("i=%s", i);
            console.log("current_node_id:%s", current_node_id);

            TreeNode memory left_node = EMPTY_NODE;
            TreeNode memory right_node = EMPTY_NODE;

            uint256 id_left = insert_node_array(left_node);
            uint256 id_right = insert_node_array(right_node);

            console.log("id_left, id_right");
            console.log(id_left);
            console.log(id_right);

            // Link the nodes between each other
            update_link_node_down(current_node_id, id_left, id_right);
            update_link_node_up(id_left, current_node_id);
            update_link_node_up(id_right, current_node_id);

            // Keep extending the tree depending on the position
            bool sib_is_left = siblings[i];

            if (sib_is_left) {
                current_node_id = id_right;
            } else {
                current_node_id = id_left;
            }

            if (i == N - 1) // final leaf node
            {
                update_val_node(current_node_id, element_hash);
            }

            i += 1;
        }

        // TODO persist the evaluation

        //logNode(ROOT_INDEX);

        // Update the hash
        TreeNode memory root = get_root();
        console.log("root.val");
        console.logBytes32(root.val);

        persist_eval_node(current_node_id);
        console.log("root_val");
        console.logBytes32(nodes[ROOT_INDEX].val);
    }

    function get_root() public returns (TreeNode memory) {
        return nodes[ROOT_INDEX];
    }

    function get_root_value() public returns (bytes32) {
        //logNode(ROOT_INDEX);
        bytes32 val = nodes[ROOT_INDEX].val;
        console.log("root_val....");
        console.logBytes32(val);
        return val;
    }

    /// Update the values of each node from a leaf to the root
    function persist_eval_node(uint256 node_id) private returns (bytes32) {
        console.log("persist_eval_node");
        uint256 current_node_id = nodes[node_id].up;

        while (true) {
            // Go one level up and update the values
            console.log("node_id: %s", current_node_id);
            bytes32 left_value = nodes[nodes[current_node_id].left].val;
            bytes32 right_value = nodes[nodes[current_node_id].right].val;
            update_val_node(
                current_node_id,
                branch_hash(left_value, right_value)
            );
            if (current_node_id == ROOT_INDEX) {
                break;
            } else {
                current_node_id = nodes[current_node_id].up;
            }
        }
    }

    function logNode(uint256 node_id) private {
        if (is_terminal_node(node_id)) {
            console.log(node_id);
        } else {
            console.log("[%s, ", node_id);
            logNode(nodes[node_id].left);
            console.log(", ");
            logNode(nodes[node_id].right);
            console.log("]");
        }
    }

    // Insert a node in the array of nodes and return the index
    function insert_node_array(TreeNode memory node) private returns (uint256) {
        uint256 id = num_nodes;
        nodes[id] = node;
        num_nodes += 1;
        return id;
    }

    function update_link_node_down(
        uint256 index_node,
        uint256 new_left,
        uint256 new_right
    ) private {
        nodes[index_node].left = new_left;
        nodes[index_node].left = new_right;
        nodes[index_node].isTerminal = false;
    }

    function update_link_node_up(uint256 index_node, uint256 index_node_up)
        private
    {
        nodes[index_node].up = index_node_up;
    }

    function update_val_node(uint256 index_node, bytes32 new_val) private {
        nodes[index_node].val = new_val;
    }

    //////

    constructor() {
        TreeNode storage root = EMPTY_NODE;
        nodes[0] = EMPTY_NODE;
        nodes[ROOT_INDEX] = root;
        num_nodes = 1;
        console.log("Root is terminal? %s", nodes[ROOT_INDEX].isTerminal);
        console.log("Root up: %s", root.up);
    }

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

        bool[N] memory sibblings = to_bool_array(element_hash);

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

        bool[N] memory sibblings = to_bool_array(element_hash);

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
