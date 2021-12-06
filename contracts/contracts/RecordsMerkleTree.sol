//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "./Rescue.sol";

/// @notice The Records Merkle Tree stores asset records.
contract RecordsMerkleTree is Rescue {
    enum Position {
        LEFT,
        MIDDLE,
        RIGHT
    }

    // Representation of a (tree) node
    // A node contains a value and pointers (which are index in an array of other nodes).
    // By convention a node that has no (left,middle,right) children will point to index 0.
    struct Node {
        uint256 val;
        uint64 left; // Pointer (index) to the left child
        uint64 middle; // Pointer (index) to the middle child
        uint64 right; // Pointer (index) to the right child
    }

    uint256 internal rootValue;
    uint64 internal numLeaves;
    uint8 internal height;

    bytes32 internal frontierHashValue;

    /// Instantiate a records merkle tree with its height
    /// @param _height height of the merkle tree
    constructor(uint8 _height) {
        rootValue = 0;
        numLeaves = 0;
        height = _height;
    }

    /// Is the given node a terminal (i.e. a leaf)?
    function isTerminal(Node memory node) private returns (bool) {
        return (node.left == 0) && (node.middle == 0) && (node.right == 0);
    }

    function hasChildren(Node memory node) private returns (bool) {
        return !isTerminal(node);
    }

    /// Is the given node null?
    function isNull(Node memory node) private returns (bool) {
        return (node.val == 0 && isTerminal(node));
    }

    /// Create the new "hole node" that points to the children already inserted in the array.
    function createHoleNode(uint64 cursor, Position posSibling)
        private
        returns (Node memory)
    {
        // Copy pasting these values to save gas
        // indexHoleNode = cursor - 3;
        // indexFirstSibling = cursor - 2;
        // indexSecondSibling = cursor - 1;

        Node memory node;
        if (posSibling == Position.LEFT) {
            node = Node(0, cursor - 3, cursor - 2, cursor - 1);
        } else if (posSibling == Position.MIDDLE) {
            node = Node(0, cursor - 2, cursor - 3, cursor - 1);
        } else if (posSibling == Position.RIGHT) {
            node = Node(0, cursor - 2, cursor - 1, cursor - 3);
        }

        return node;
    }

    function hashFrontier(uint256[] memory flattenedFrontier, uint64 uid)
        internal
        returns (bytes32)
    {
        uint256 frontierLength = flattenedFrontier.length;
        uint256[] memory input = new uint256[](frontierLength + 1);
        input[0] = uint256(uid);
        for (uint256 i = 0; i < frontierLength; i++) {
            input[i + 1] = flattenedFrontier[i];
        }

        bytes32 value = keccak256(abi.encode(input));

        return value;
    }

    /// Checks that the frontier represented as a tree resolves to the right root
    /// @param flattenedFrontier "flat" representation of the frontier. Note that the frontier is fully defined with the flattened representation and the current number of leaves.
    /// @return true if hashing the flattened frontier concatenated with the number of leaves equal the hash value of the frontier stored in the contract.
    function checkFrontier(uint256[] memory flattenedFrontier)
        internal
        returns (bool)
    {
        if (flattenedFrontier.length == 0) {
            // When the tree is empty
            return frontierHashValue == 0;
        } else {
            // Compute the hash of the frontier
            bytes32 computedFrontierHashValue = hashFrontier(
                flattenedFrontier,
                numLeaves - 1
            );

            return computedFrontierHashValue == frontierHashValue;
        }
    }

    /// Builds a Merkle tree from a frontier.
    /// Returns a cursor.
    function buildTreeFromFrontier(
        uint256[] memory flattenedFrontier,
        Node[] memory nodes
    ) internal returns (uint64) {
        // Tree is empty
        if (flattenedFrontier.length == 0) {
            nodes[0] = Node(0, 0, 0, 0); // Empty node
            nodes[1] = Node(0, 0, 0, 0); // Root node
            return 1;
        }
        // Tree is not empty

        // Set the first node to the NULL node
        nodes[0] = Node(0, 0, 0, 0);

        // Insert the leaf
        nodes[1] = Node(flattenedFrontier[0], 0, 0, 0);

        // Insert the siblings
        nodes[2] = Node(flattenedFrontier[1], 0, 0, 0);
        nodes[3] = Node(flattenedFrontier[2], 0, 0, 0);

        // Compute the position of each node
        uint64 absolutePosition = numLeaves - 1;
        uint8 localPosition = uint8(absolutePosition % 3);

        // We process the nodes of the Merkle path
        uint64 cursor = 4;
        uint64 cursorFrontier = 3;

        // Build the tree expect the root node
        while (cursor < 3 * height + 1) {
            nodes[cursor] = createHoleNode(cursor, Position(localPosition));

            // Create the siblings of the "hole node". These siblings have no children
            nodes[cursor + 1] = Node(
                flattenedFrontier[cursorFrontier],
                0,
                0,
                0
            );
            nodes[cursor + 2] = Node(
                flattenedFrontier[cursorFrontier + 1],
                0,
                0,
                0
            );

            // Move forward
            absolutePosition /= 3;
            localPosition = uint8(absolutePosition % 3);

            cursor += 3;
            cursorFrontier += 2;
        }

        // Add the root node
        nodes[cursor] = createHoleNode(cursor, Position(localPosition));
        return cursor;
    }

    function nextNodeIndex(
        Node[] memory nodes,
        uint64 nodeIndex,
        Position pos
    ) private returns (uint64) {
        uint64 res;

        if (pos == Position.LEFT) {
            res = nodes[nodeIndex].left;
        } else if (pos == Position.MIDDLE) {
            res = nodes[nodeIndex].middle;
        } else if (pos == Position.RIGHT) {
            res = nodes[nodeIndex].right;
        }

        return res;
    }

    // Update the child of a node based on the position (which child to select)
    // and an index to the new child.
    function updateChildNode(
        Node memory node,
        uint64 newChildIndex,
        Position pos
    ) private {
        // Update the node
        if (pos == Position.LEFT) {
            node.left = newChildIndex;
        } else if (pos == Position.MIDDLE) {
            node.middle = newChildIndex;
        } else if (pos == Position.RIGHT) {
            node.right = newChildIndex;
        }
    }

    function computeNodePos(uint64 absolutePos, uint64 branchIndex)
        private
        returns (uint64, uint64)
    {
        uint64 localPos;
        uint64 divisor = uint64(3**(height - branchIndex - 1));

        localPos = absolutePos / divisor;
        absolutePos = absolutePos % divisor;

        return (absolutePos, localPos);
    }

    /// Insert an element into the tree in the position num_leaves
    /// @param nodes array of nodes
    /// @param rootIndex index of the root node
    /// @param maxIndex index of the latest element inserted in the nodes array
    /// @param element value of the element to insert into the tree
    /// @return updated value of maxIndex
    function pushElement(
        Node[] memory nodes,
        uint64 rootIndex,
        uint64 maxIndex,
        uint256 element
    ) private returns (uint64) {
        require(numLeaves < 3**height, "The tree is full.");

        // Get the position of the leaf from the smart contract state
        uint64 leafPos = numLeaves;
        uint64 branchIndex = 0;
        uint64 currentNodeIndex = rootIndex;
        uint64 previousNodeIndex = rootIndex;

        // Go down inside the tree until finding the first terminal node.
        uint64 absolutePos = leafPos;
        uint64 localPos = 0;
        while (!isNull(nodes[currentNodeIndex])) {
            (absolutePos, localPos) = computeNodePos(absolutePos, branchIndex);

            previousNodeIndex = currentNodeIndex;
            currentNodeIndex = nextNodeIndex(
                nodes,
                currentNodeIndex,
                Position(localPos)
            );

            branchIndex += 1;
        }

        // maxIndex tracks the index of the last element inserted in the tree
        uint64 newNodeIndex = maxIndex + 1;

        // Create new nodes until completing the path one level above the leaf level
        // Always inserting to the left

        // To compensate the extra increment at the end of the previous loop ,
        // except if the tree is reduced to a single root node.
        if (branchIndex > 0) {
            branchIndex -= 1;
        }

        while (branchIndex < height - 1) {
            nodes[newNodeIndex] = Node(0, 0, 0, 0);
            updateChildNode(
                nodes[previousNodeIndex],
                newNodeIndex,
                Position(localPos)
            );

            // Prepare the next iteration of the loop
            previousNodeIndex = newNodeIndex;
            newNodeIndex += 1;
            branchIndex += 1;
            (absolutePos, localPos) = computeNodePos(absolutePos, branchIndex);
        }

        // The last node contains the leaf value (compute the hash)
        // Remember position is computed with the remainder

        // Leaf node where the value is hash(0,numLeaves,element)
        uint256 val = hash(0, numLeaves, element);
        nodes[newNodeIndex] = Node(val, 0, 0, 0);
        updateChildNode(
            nodes[previousNodeIndex],
            newNodeIndex,
            Position(localPos)
        );

        // Increment the number of leaves
        numLeaves += 1;

        // Return the new value of maxIndex
        return newNodeIndex;
    }

    /// Updates the hash of the frontier based on the current tree structure.
    function updateFrontierHash(Node[] memory nodes, uint64 rootIndex) private {
        /// Update the hash of the frontier
        uint64 frontierSize = 2 * height + 1;
        uint256[] memory newFlattenedFrontier = new uint256[](frontierSize);

        /// Collect the values from the root to the leaf but in reverse order
        uint64 currentNodeIndex = rootIndex;
        uint64 firstSiblingIndex = 0;
        uint64 secondSiblingIndex = 0;
        // Go down until the leaf
        for (uint256 i = 0; i < height; i++) {
            // Pick the non-empty node that is most right
            Node memory currentNode = nodes[currentNodeIndex];
            if (!isNull(nodes[currentNode.right])) {
                // Keep to the right
                currentNodeIndex = currentNode.right;
                firstSiblingIndex = currentNode.left;
                secondSiblingIndex = currentNode.middle;
            } else if (!isNull(nodes[currentNode.middle])) {
                // Keep to the middle
                currentNodeIndex = currentNode.middle;
                firstSiblingIndex = currentNode.left;
                secondSiblingIndex = currentNode.right;
            } else {
                // Keep to the left
                currentNodeIndex = currentNode.left;
                firstSiblingIndex = currentNode.middle;
                secondSiblingIndex = currentNode.right;
            }
            uint256 secondSiblingPos = frontierSize - 1 - (2 * i);
            uint256 firstSiblingPos = secondSiblingPos - 1;
            newFlattenedFrontier[secondSiblingPos] = nodes[secondSiblingIndex]
                .val;
            newFlattenedFrontier[firstSiblingPos] = nodes[firstSiblingIndex]
                .val;
        }
        // currentNodeIndex points to the leaf
        newFlattenedFrontier[0] = nodes[currentNodeIndex].val;

        frontierHashValue = hashFrontier(newFlattenedFrontier, numLeaves - 1);
    }

    /// Updates the state of the record merkle tree by inserting new elements
    /// @param flattenedFrontier list composed by the leaf of the frontier and the list of siblings from bottom to top (the root). Note that the path is already encoded implicitly with the number of leaves *numLeaves* stored in the contract,
    /// @param elements list of elements to be appended to the current merkle tree described by the frontier.
    function updateRecordsMerkleTree(
        uint256[] memory flattenedFrontier,
        uint256[] memory elements
    ) internal {
        // The total number of nodes is bounded by 3*height+1 + 3*N*height = 3*(N+1)*height + 1
        // where N is the number of new records
        uint256 numElements = elements.length;
        Node[] memory nodes = new Node[](3 * (numElements + 1) * height + 2);

        bool isFrontierValid = checkFrontier(flattenedFrontier);
        require(isFrontierValid, "Frontier not consistent w/ state");

        /// Insert the new elements ///

        if (elements.length > 0) {
            // maxIndex tracks the index of the last element inserted in the tree
            uint64 rootIndex = buildTreeFromFrontier(flattenedFrontier, nodes);
            uint64 maxIndex = rootIndex;
            for (uint32 i = 0; i < elements.length; i++) {
                maxIndex = pushElement(nodes, rootIndex, maxIndex, elements[i]);
            }
            //// Compute the root hash value ////
            rootValue = computeRootValueAndUpdateTree(nodes, rootIndex);

            //// Update the frontier hash
            updateFrontierHash(nodes, rootIndex);
        }
    }

    // Returns the root value of the Merkle tree
    function getRootValue() public view returns (uint256) {
        return rootValue;
    }

    /// Updates the tree by hashing the children of each node.
    /// @param nodes tree structure. Note that the nodes are updated by this function.
    /// @param rootNodePos index of the root node in the list of nodes.
    /// @return the value obtained at the root.
    function computeRootValueAndUpdateTree(
        Node[] memory nodes,
        uint256 rootNodePos
    ) private returns (uint256) {
        // If the root node has no children return its value
        Node memory rootNode = nodes[rootNodePos];
        if (isTerminal(rootNode)) {
            return rootNode.val;
        } else {
            uint256 valLeft = computeRootValueAndUpdateTree(
                nodes,
                rootNode.left
            );
            uint256 valMiddle = computeRootValueAndUpdateTree(
                nodes,
                rootNode.middle
            );
            uint256 valRight = computeRootValueAndUpdateTree(
                nodes,
                rootNode.right
            );

            nodes[rootNode.left].val = valLeft;
            nodes[rootNode.middle].val = valMiddle;
            nodes[rootNode.right].val = valRight;

            return hash(valLeft, valMiddle, valRight);
        }
    }
}
