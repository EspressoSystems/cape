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

    uint256 public constant MAX_NUMBER_NODES = 100; // TODO precise number depending on tree height
    uint256 public constant EMPTY_NODE_INDEX = 0;
    uint256 public constant EMPTY_NODE_VALUE = 0;
    uint64 public constant HEIGHT = 25; // TODO set this value with the constructor

    uint256 public constant LEAF_INDEX = 1;

    uint256 internal rootValue;
    uint64 internal numLeaves;

    constructor() {
        rootValue = EMPTY_NODE_VALUE;
    }

    function isTerminal(Node memory node) private returns (bool) {
        return
            (node.left == EMPTY_NODE_INDEX) &&
            (node.middle == EMPTY_NODE_INDEX) &&
            (node.right == EMPTY_NODE_INDEX);
    }

    function isNull(Node memory node) private returns (bool) {
        return (node.val == EMPTY_NODE_VALUE && isTerminal(node));
    }

    // Create the new "hole node" that points to the children already inserted in the array
    function createHoleNode(
        uint256 indexNodesArray,
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 indexHoleNode,
        uint256 indexFirstSibling,
        uint256 indexSecondSibling,
        uint256 posSibling
    ) private {
        if (posSibling == 0) {
            // TODO use constants for LEFT=0, MIDDLE=1, RIGHT=2
            nodes[indexNodesArray] = Node(
                0,
                indexHoleNode,
                indexFirstSibling,
                indexSecondSibling
            );
        }
        if (posSibling == 1) {
            nodes[indexNodesArray] = Node(
                0,
                indexFirstSibling,
                indexHoleNode,
                indexSecondSibling
            );
        }
        if (posSibling == 2) {
            nodes[indexNodesArray] = Node(
                0,
                indexFirstSibling,
                indexSecondSibling,
                indexHoleNode
            );
        }
    }

    /// Checks that the frontier represented as a tree resolves to the right root and number of leaves
    /// @param nodes array of nodes obtained from the frontier
    /// @return true if the tree resolves to right root_value and num_leaves
    function checkFrontier(
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 rootIndex
    ) private returns (bool) {
        // Compute the root value of the frontier
        uint256 frontierRootValue = computeRootValue(nodes, rootIndex);

        console.log("root_value %s", rootValue);
        console.log("frontier_root_value %s", frontierRootValue);

        // Compute the number of leaves from the frontier represented as nodes
        uint256 numLeavesFromFrontier = 0;

        uint256 index = rootIndex;
        Node memory node = nodes[rootIndex];

        // We are done when we reach the leaf. The leaf index is LEAF_INDEX.
        // See function build_tree_from_frontier.
        uint256 powerOfThree = 3**(HEIGHT - 1);
        while (index != LEAF_INDEX) {
            if (!isNull(nodes[node.left])) {
                index = node.left;
            }
            if (!isNull(nodes[node.middle])) {
                numLeavesFromFrontier += powerOfThree * 1;
                index = node.middle;
            }
            if (!isNull(nodes[node.right])) {
                numLeavesFromFrontier += powerOfThree * 2;
                index = node.right;
            }
            powerOfThree /= 3;
            console.log("index: %s", index);
            node = nodes[index];
        }

        numLeavesFromFrontier += 1;

        console.log("expected_number_of_leaves: %s", numLeavesFromFrontier);
        console.log("num_leaves: %s", numLeaves);

        return
            (frontierRootValue == rootValue) &&
            (numLeavesFromFrontier == numLeaves);
    }

    function buildTreeFromFrontier(
        uint256[] memory _frontier,
        Node[MAX_NUMBER_NODES] memory nodes
    ) private returns (uint256) {
        // Set the first node to the NULL node
        nodes[0] = Node(0, 0, 0, 0); // N

        // Insert the leaf
        nodes[LEAF_INDEX] = Node(_frontier[0], 0, 0, 0);

        // Insert the siblings
        uint256 indexFirstSibling = 2;
        nodes[indexFirstSibling] = Node(_frontier[1], 0, 0, 0);
        uint256 indexSecondSibling = 3;
        nodes[indexSecondSibling] = Node(_frontier[2], 0, 0, 0);

        uint256 posSibling = _frontier[3];

        // We process the nodes of the Merkle path
        uint256 indexNodesArray = 3;
        uint256 indexFrontier = 4;
        uint256 indexHoleNode = LEAF_INDEX;
        uint256 frontierLen = _frontier.length; // TODO This should be constant
        while (indexFrontier < frontierLen) {
            indexNodesArray += 1;
            createHoleNode(
                indexNodesArray,
                nodes,
                indexHoleNode,
                indexFirstSibling,
                indexSecondSibling,
                posSibling
            );

            // Update the index of the hole node for the next iteration
            indexHoleNode = indexNodesArray;

            // Create the siblings of the "hole node". These siblings have no children
            indexNodesArray += 1;
            indexFirstSibling = indexNodesArray;
            nodes[indexFirstSibling] = Node(_frontier[indexFrontier], 0, 0, 0);

            indexNodesArray += 1;
            indexSecondSibling = indexNodesArray;
            nodes[indexSecondSibling] = Node(
                _frontier[indexFrontier + 1],
                0,
                0,
                0
            );

            posSibling = _frontier[indexFrontier + 2];

            // Move forward
            indexFrontier = indexFrontier + 3;
        }

        // Add the root node
        indexNodesArray += 1;
        createHoleNode(
            indexNodesArray,
            nodes,
            indexHoleNode,
            indexFirstSibling,
            indexSecondSibling,
            posSibling
        );

        return indexNodesArray;
    }

    // TODO document, in particular how the _frontier is built
    function updateRecordsMerkleTree(
        uint256[] memory _frontier,
        uint256[] memory _elements
    ) internal {
        Node[MAX_NUMBER_NODES] memory nodes;

        uint256 rootIndex = buildTreeFromFrontier(_frontier, nodes);
        bool isFrontierValid = checkFrontier(nodes, rootIndex);
        require(isFrontierValid, "Frontier not consistent state.");

        /// Insert the new elements ///
        // TODO
        if (_elements.length == 0) {
            console.log("empty");
        }

        //// Compute the root hash value ////
        rootValue = computeRootValue(nodes, rootIndex);
    }

    function getRootValue() public view returns (uint256) {
        return rootValue;
    }

    function computeRootValue(
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 rootNodePos
    ) private returns (uint256) {
        // If the root node has no children return its value
        Node memory rootNode = nodes[rootNodePos];
        if (isTerminal(rootNode)) {
            return rootNode.val;
        } else {
            uint256 valLeft = computeRootValue(nodes, rootNode.left);
            uint256 valMiddle = computeRootValue(nodes, rootNode.middle);
            uint256 valRight = computeRootValue(nodes, rootNode.right);

            return hash(valLeft, valMiddle, valRight);
        }
    }
}
