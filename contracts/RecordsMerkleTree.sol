//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "./Rescue.sol";

contract RecordsMerkleTree is Rescue {
    // Representation of a (tree) node
    // A node contains a value and pointers (which are index in an array of other nodes).
    // By convention a node that has no (left,middle,right) children will point to index 0.
    struct Node {
        uint256 val;
        uint256 left; // Pointer (index) to the left child
        uint256 middle; // Pointer (index) to the middle child
        uint256 right; // Pointer (index) to the right child
    }

    // TODO index value of array should be u64 or u32

    uint256 constant MAX_NUMBER_NODES = 100; // TODO precise number depending on tree height
    uint256 constant EMPTY_NODE_INDEX = 0;
    uint256 constant EMPTY_NODE_VALUE = 0;
    uint64 constant HEIGHT = 25; // TODO set this value with the constructor

    uint256 constant LEAF_INDEX = 1;

    uint256 internal root_value;
    uint64 internal num_leaves;

    constructor() {
        root_value = EMPTY_NODE_VALUE;
    }

    function is_terminal(Node memory node) private returns (bool) {
        return
            (node.left == EMPTY_NODE_INDEX) &&
            (node.middle == EMPTY_NODE_INDEX) &&
            (node.right == EMPTY_NODE_INDEX);
    }

    function is_null(Node memory node) private returns (bool) {
        return (node.val == EMPTY_NODE_VALUE && is_terminal(node));
    }

    // Create the new "hole node" that points to the children already inserted in the array
    function create_hole_node(
        uint256 index_nodes_array,
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 index_hole_node,
        uint256 index_first_sibling,
        uint256 index_second_sibling,
        uint256 pos_sibling
    ) private {
        if (pos_sibling == 0) {
            // TODO use constants for LEFT=0, MIDDLE=1, RIGHT=2
            nodes[index_nodes_array] = Node(
                0,
                index_hole_node,
                index_first_sibling,
                index_second_sibling
            );
        }
        if (pos_sibling == 1) {
            nodes[index_nodes_array] = Node(
                0,
                index_first_sibling,
                index_hole_node,
                index_second_sibling
            );
        }
        if (pos_sibling == 2) {
            nodes[index_nodes_array] = Node(
                0,
                index_first_sibling,
                index_second_sibling,
                index_hole_node
            );
        }
    }

    /// Checks that the frontier represented as a tree resolves to the right root and number of leaves
    /// @param nodes array of nodes obtained from the frontier
    /// @return true if the tree resolves to right root_value and num_leaves
    function check_frontier(
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 root_index
    ) private returns (bool) {
        // Compute the root value of the frontier
        uint256 frontier_root_value = compute_root_value(nodes, root_index);

        console.log("root_value %s", root_value);
        console.log("frontier_root_value %s", frontier_root_value);

        // Compute the number of leaves from the frontier represented as nodes
        uint256 num_leaves_from_frontier = 0;

        uint256 index = root_index;
        Node memory node = nodes[root_index];

        // We are done when we reach the leaf. The leaf index is LEAF_INDEX.
        // See function build_tree_from_frontier.
        uint256 power_of_three = 3**(HEIGHT - 1);
        while (index != LEAF_INDEX) {
            if (!is_null(nodes[node.left])) {
                index = node.left;
            }
            if (!is_null(nodes[node.middle])) {
                num_leaves_from_frontier += power_of_three * 1;
                index = node.middle;
            }
            if (!is_null(nodes[node.right])) {
                num_leaves_from_frontier += power_of_three * 2;
                index = node.right;
            }
            power_of_three /= 3;
            console.log("index: %s", index);
            node = nodes[index];
        }

        num_leaves_from_frontier += 1;

        console.log("expected_number_of_leaves: %s", num_leaves_from_frontier);
        console.log("num_leaves: %s", num_leaves);

        return
            (frontier_root_value == root_value) &&
            (num_leaves_from_frontier == num_leaves);
    }

    function build_tree_from_frontier(
        uint256[] memory _frontier,
        Node[MAX_NUMBER_NODES] memory nodes
    ) private returns (uint256) {
        uint256 index_nodes_array = 0;

        // Set the first node to the NULL node
        Node memory NULL_NODE = Node(0, 0, 0, 0);
        nodes[index_nodes_array] = NULL_NODE;

        // Insert the leaf
        Node memory leaf_node = Node(_frontier[0], 0, 0, 0);

        index_nodes_array += 1;
        nodes[LEAF_INDEX] = leaf_node;

        // Now we process the siblings of the leaf
        index_nodes_array += 1;
        uint256 index_first_sibling = index_nodes_array;
        nodes[index_first_sibling] = Node(_frontier[1], 0, 0, 0);

        index_nodes_array += 1;
        uint256 index_second_sibling = index_nodes_array;
        nodes[index_second_sibling] = Node(_frontier[2], 0, 0, 0);

        uint256 pos_sibling = _frontier[3];

        // We process the nodes of the Merkle path
        uint256 index_frontier = 4;
        uint256 index_hole_node = LEAF_INDEX;
        uint256 frontier_len = _frontier.length; // TODO This should be constant
        while (index_frontier < frontier_len) {
            index_nodes_array += 1;
            create_hole_node(
                index_nodes_array,
                nodes,
                index_hole_node,
                index_first_sibling,
                index_second_sibling,
                pos_sibling
            );

            // Update the index of the hole node for the next iteration
            index_hole_node = index_nodes_array;

            // Create the siblings of the "hole node". These siblings have no children
            index_nodes_array += 1;
            index_first_sibling = index_nodes_array;
            nodes[index_first_sibling] = Node(
                _frontier[index_frontier],
                0,
                0,
                0
            );

            index_nodes_array += 1;
            index_second_sibling = index_nodes_array;
            nodes[index_second_sibling] = Node(
                _frontier[index_frontier + 1],
                0,
                0,
                0
            );

            pos_sibling = _frontier[index_frontier + 2];

            // Move forward
            index_frontier = index_frontier + 3;
        }

        // Add the root node
        index_nodes_array += 1;
        create_hole_node(
            index_nodes_array,
            nodes,
            index_hole_node,
            index_first_sibling,
            index_second_sibling,
            pos_sibling
        );

        return index_nodes_array;
    }

    // TODO document, in particular how the _frontier is built
    function update_records_merkle_tree(
        uint256[] memory _frontier,
        uint256[] memory _elements
    ) internal {
        Node[MAX_NUMBER_NODES] memory nodes;

        uint256 root_index = build_tree_from_frontier(_frontier, nodes);
        bool is_frontier_valid = check_frontier(nodes, root_index);
        require(
            is_frontier_valid,
            "The frontier is not consistent with the root value and/or number of leaves."
        );

        /// Insert the new elements ///
        // TODO

        //// Compute the root hash value ////
        root_value = compute_root_value(nodes, root_index);
    }

    function get_root_value() public view returns (uint256) {
        return root_value;
    }

    function compute_root_value(
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 root_node_pos
    ) private returns (uint256) {
        // If the root node has no children return its value
        Node memory root_node = nodes[root_node_pos];
        if (is_terminal(root_node)) {
            return root_node.val;
        } else {
            uint256 val_left = compute_root_value(nodes, root_node.left);
            uint256 val_middle = compute_root_value(nodes, root_node.middle);
            uint256 val_right = compute_root_value(nodes, root_node.right);

            return hash(val_left, val_middle, val_right);
        }
    }
}
