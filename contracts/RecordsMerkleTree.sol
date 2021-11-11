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

    enum Position {
        LEFT,
        MIDDLE,
        RIGHT
    }

    // TODO index value of array should be u64 or u32

    uint256 public constant MAX_NUMBER_NODES = 100; // TODO precise number depending on tree height
    uint256 public constant EMPTY_NODE_INDEX = 0;
    uint256 public constant EMPTY_NODE_VALUE = 0;

    uint256 public constant LEAF_INDEX = 1;

    uint256 internal rootValue;
    uint64 internal numLeaves;
    uint64 internal height;

    constructor(uint64 _height) {
        rootValue = EMPTY_NODE_VALUE;
        numLeaves = 0;
        height = _height;
    }

    function isTerminal(Node memory node) private returns (bool) {
        return
            (node.left == EMPTY_NODE_INDEX) &&
            (node.middle == EMPTY_NODE_INDEX) &&
            (node.right == EMPTY_NODE_INDEX);
    }

    // TODO save gas using comparison against a constant node value Node(0,0,0,0)?
    function isNull(Node memory node) private returns (bool) {
        return (node.val == EMPTY_NODE_VALUE && isTerminal(node));
    }

    // Create the new "hole node" that points to the children already inserted in the array
    function createHoleNode(uint256 cursor, Position posSibling)
        private
        returns (Node memory)
    {
        uint256 indexHoleNode = cursor - 3;
        uint256 indexFirstSibling = cursor - 2;
        uint256 indexSecondSibling = cursor - 1;

        uint256 left;
        uint256 middle;
        uint256 right;

        if (posSibling == Position.LEFT) {
            left = indexHoleNode;
            middle = indexFirstSibling;
            right = indexSecondSibling;
        } else if (posSibling == Position.MIDDLE) {
            left = indexFirstSibling;
            middle = indexHoleNode;
            right = indexSecondSibling;
        } else if (posSibling == Position.RIGHT) {
            left = indexFirstSibling;
            middle = indexSecondSibling;
            right = indexHoleNode;
        }

        return Node(0, left, middle, right);
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

        uint256 branchIndex = 0;
        uint256 nodeIndex = rootIndex;
        Node memory node = nodes[rootIndex];

        // We are done when we reach the leaf. The leaf index is LEAF_INDEX.
        // See function buildTreeFromFrontier.
        uint256 powerOfThree = 3**(height - 1);
        while (branchIndex < height) {
            console.log("powerOfThree: %s", powerOfThree);
            if (!isNull(nodes[node.left]) && isNull(nodes[node.middle])) {
                nodeIndex = node.left;
                console.log("LEFT");
            }
            if (!isNull(nodes[node.middle]) && isNull(nodes[node.right])) {
                numLeavesFromFrontier += powerOfThree * 1;
                nodeIndex = node.middle;
                console.log("MIDDLE");
            }
            if (!isNull(nodes[node.right])) {
                numLeavesFromFrontier += powerOfThree * 2;
                nodeIndex = node.right;
                console.log("RIGHT");
            }
            powerOfThree /= 3;
            console.log("index: %s", nodeIndex);
            branchIndex += 1;
            node = nodes[nodeIndex];
        }

        // The previous loop computes the index of the leaf.
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
        nodes[0] = Node(0, 0, 0, 0);

        // Insert the leaf
        nodes[1] = Node(_frontier[0], 0, 0, 0);

        // Insert the siblings
        nodes[2] = Node(_frontier[1], 0, 0, 0);
        nodes[3] = Node(_frontier[2], 0, 0, 0);

        // We process the nodes of the Merkle path
        uint256 cursor = 4;

        // The length of the frontier is
        while (cursor < 3 * height + 1) {
            nodes[cursor] = createHoleNode(
                cursor,
                Position(_frontier[cursor - 1])
            );

            // Create the siblings of the "hole node". These siblings have no children
            nodes[cursor + 1] = Node(_frontier[cursor], 0, 0, 0);
            nodes[cursor + 2] = Node(_frontier[cursor + 1], 0, 0, 0);

            // Move forward
            cursor += 3;
        }

        // Add the root node
        nodes[cursor] = createHoleNode(cursor, Position(_frontier[cursor - 1]));

        return cursor;
    }

    function nextNodeIndex(
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 nodeIndex,
        Position pos
    ) private returns (uint256) {
        uint256 nextNodeIndex;

        if (pos == Position.LEFT) {
            nextNodeIndex = nodes[nodeIndex].left;
        } else if (pos == Position.MIDDLE) {
            nextNodeIndex = nodes[nodeIndex].middle;
        } else if (pos == Position.RIGHT) {
            nextNodeIndex = nodes[nodeIndex].right;
        }

        return nextNodeIndex;
    }

    // Update the child of a node based on the position (which child to select)
    // and an index to the new child.
    function updateChildNode(
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 nodeIndex,
        uint256 newChildIndex,
        Position pos
    ) private {
        // Get the node
        Node memory node = nodes[nodeIndex];

        // Update the node
        if (pos == Position.LEFT) {
            node.left = newChildIndex;
        } else if (pos == Position.MIDDLE) {
            node.middle = newChildIndex;
        } else if (pos == Position.RIGHT) {
            node.right = newChildIndex;
        }

        console.log(
            "Child with index %s and position %s of node with index %s updated.",
            newChildIndex,
            uint256(pos),
            nodeIndex
        );
    }

    // TODO is it possible to create a data structure for handling the nodes array and tracking the maximum index at
    // TODO the same time? Tracking maxIndex outside the "nodes collection" is error prone.

    /// Insert an element into the tree in the position num_leaves
    /// @param nodes array of nodes
    /// @param rootIndex index of the root node
    /// @param maxIndex index of the latest element inserted in the nodes array
    /// @param element value of the element to insert into the tree
    /// @return updated value of maxIndex
    function pushElement(
        Node[MAX_NUMBER_NODES] memory nodes,
        uint256 rootIndex,
        uint256 maxIndex,
        uint256 element
    ) private returns (uint256) {
        console.log("height: %s", height);
        console.log("num_leaves: %s", numLeaves);
        console.log("element: %s", element);

        // Get the position of the leaf from the smart contract state
        uint256 leafPos = numLeaves;
        uint256 branchIndex = 0;
        uint256 currentNodeIndex = rootIndex;
        uint256 previousNodeIndex = rootIndex;

        // Go down inside the tree until finding the first terminal node.
        console.log("Going down until finding a terminal node");
        uint256 pos = leafPos;
        uint256 localPos = 0;
        while (!isNull(nodes[currentNodeIndex])) {
            console.log(
                "Going down one position from node with index %s",
                currentNodeIndex
            );

            // TODO avoid this logic duplication?
            uint256 divisor = 3**(height - branchIndex - 1);
            localPos = pos / divisor;
            pos = pos % divisor;

            console.log("branchIndex: %s", branchIndex);
            console.log("currentNodeIndex: %s", currentNodeIndex);
            console.log("previousNodeIndex: %s", previousNodeIndex);
            console.log("localPos: %s", localPos);

            previousNodeIndex = currentNodeIndex;
            currentNodeIndex = nextNodeIndex(
                nodes,
                currentNodeIndex,
                Position(localPos)
            );

            if (isNull(nodes[currentNodeIndex])) {
                console.log(
                    "Node with index %s is terminal.",
                    currentNodeIndex
                );
                console.log("Previous node index is %s", previousNodeIndex);

                // Update previousNode pointer and localPos
                if (branchIndex < height - 1) {
                    previousNodeIndex = currentNodeIndex;
                    // TODO avoid this logic duplication?
                    uint256 divisor = 3**(height - branchIndex - 1);
                    localPos = pos / divisor;
                }
            }
            branchIndex += 1;
        }

        // maxIndex tracks the index of the last element inserted in the tree
        uint256 newNodeIndex = maxIndex + 1;

        // Create new nodes until completing the path one level above the leaf level
        // Always inserting to the left
        console.log("Create new nodes");
        console.log("branchIndex: %s", branchIndex);

        while (branchIndex < height - 1) {
            // New node
            console.log("Adding new node with index: %s", newNodeIndex);
            console.log("branchIndex: %s", branchIndex);
            nodes[newNodeIndex] = Node(0, 0, 0, 0);

            // TODO avoid this logic duplication?
            uint256 divisor = 3**(height - branchIndex - 1);
            localPos = pos / divisor;
            pos = pos % divisor;

            console.log("localPos: %s", localPos);

            updateChildNode(
                nodes,
                previousNodeIndex,
                newNodeIndex,
                Position(localPos)
            );

            previousNodeIndex = newNodeIndex;
            newNodeIndex += 1;
            branchIndex += 1;
        }

        // The last node contains the leaf value (compute the hash)
        // Remember position is computed with the remainder
        console.log("adding the leaf");

        // Leaf node where the value is hash(0,numLeaves,element)
        nodes[newNodeIndex] = Node(
            hash(EMPTY_NODE_VALUE, numLeaves, element),
            0,
            0,
            0
        );

        console.log("Leaf level position: %s", localPos);
        console.log("The leaf index is %s.", newNodeIndex);

        updateChildNode(
            nodes,
            previousNodeIndex,
            newNodeIndex,
            Position(localPos)
        );

        console.log(
            "The children ids of the previous node with id %s are:",
            previousNodeIndex
        );
        console.log(
            "[%s,%s,%s]",
            nodes[previousNodeIndex].left,
            nodes[previousNodeIndex].middle,
            nodes[previousNodeIndex].right
        );

        // Increment the number of leaves
        numLeaves += 1;

        // Return the new value of maxIndex
        return newNodeIndex;
    }

    // TODO document, in particular how the _frontier is built
    function updateRecordsMerkleTree(
        uint256[] memory _frontier,
        uint256[] memory _elements
    ) internal {
        Node[MAX_NUMBER_NODES] memory nodes;

        uint256 rootIndex = buildTreeFromFrontier(_frontier, nodes);
        bool isFrontierValid = checkFrontier(nodes, rootIndex);
        require(isFrontierValid, "Frontier not consistent w/ state");

        /// Insert the new elements ///

        // maxIndex tracks the index of the last element inserted in the tree
        uint256 maxIndex = rootIndex;
        for (uint256 i = 0; i < _elements.length; i++) {
            maxIndex = pushElement(nodes, rootIndex, maxIndex, _elements[i]);
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
