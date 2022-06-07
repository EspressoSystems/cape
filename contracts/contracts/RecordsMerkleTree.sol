// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import "./libraries/RescueLib.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract RecordsMerkleTree is Ownable {
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

    uint256 internal _rootValue;
    uint64 internal _numLeaves;
    uint8 internal _merkleTreeHeight;

    mapping(uint256 => uint256) internal _flattenedFrontier;

    /// @dev Create a records Merkle tree of the given height.
    /// @param merkleTreeHeight The height
    constructor(uint8 merkleTreeHeight) {
        _rootValue = 0;
        _numLeaves = 0;
        _merkleTreeHeight = merkleTreeHeight;
    }

    /// @dev Is the given node a terminal node?
    /// @param node A node
    /// @return _ True if the node is terminal, false otherwise.
    function _isTerminal(Node memory node) private pure returns (bool) {
        return (node.left == 0) && (node.middle == 0) && (node.right == 0);
    }

    /// @dev Does the given node have children?
    /// @param node A node
    /// @return _ True if the node has at least one child, false otherwise
    function _hasChildren(Node memory node) private pure returns (bool) {
        return !_isTerminal(node);
    }

    /// @dev Is the given node null?
    /// @param node A node
    /// @return _ True if the node is NULL, false otherwise
    function _isNull(Node memory node) private pure returns (bool) {
        return (node.val == 0 && _isTerminal(node));
    }

    /// @dev Create a new "hole node" at the given position in the
    /// tree. A cursor position can be obtained from an extant node or
    /// from a function that returns a position such as _buildTreeFromFrontier.
    /// @param cursor The index of the node in the array of nodes
    /// @param posSibling The position of the sibling i.e. (LEFT, MIDDLE or RIGHT)
    /// @return _ The new created node
    function _createHoleNode(uint64 cursor, Position posSibling)
        private
        pure
        returns (Node memory)
    {
        // Copy pasting these values to save gas
        // indexHoleNode = cursor - 3;
        // indexFirstSibling = cursor - 2;
        // indexSecondSibling = cursor - 1;

        if (posSibling == Position.LEFT) {
            return Node(0, cursor - 3, cursor - 2, cursor - 1);
        } else if (posSibling == Position.MIDDLE) {
            return Node(0, cursor - 2, cursor - 3, cursor - 1);
        } else if (posSibling == Position.RIGHT) {
            return Node(0, cursor - 2, cursor - 1, cursor - 3);
        } else {
            revert("unreachable");
        }
    }

    /// @dev Create a Merkle tree from the given frontier.
    /// @param nodes The list of nodes to be filled or updated
    /// @return A cursor to the root node of the create tree
    function _buildTreeFromFrontier(Node[] memory nodes) internal view returns (uint64) {
        // Tree is empty
        if (_numLeaves == 0) {
            nodes[0] = Node(0, 0, 0, 0); // Empty node
            nodes[1] = Node(0, 0, 0, 0); // Root node
            return 1;
        }

        // Tree is not empty

        // Set the first node to the NULL node
        nodes[0] = Node(0, 0, 0, 0);

        // Insert the leaf
        nodes[1] = Node(_flattenedFrontier[0], 0, 0, 0);

        // Insert the siblings
        nodes[2] = Node(_flattenedFrontier[1], 0, 0, 0);
        nodes[3] = Node(_flattenedFrontier[2], 0, 0, 0);

        // Compute the position of each node
        uint64 absolutePosition = _numLeaves - 1;
        uint8 localPosition = uint8(absolutePosition % 3);

        // We process the nodes of the Merkle path
        uint64 cursor = 4;
        uint64 cursorFrontier = 3;

        // Build the tree expect the root node
        while (cursor < 3 * _merkleTreeHeight + 1) {
            nodes[cursor] = _createHoleNode(cursor, Position(localPosition));

            // Create the siblings of the "hole node". These siblings have no children
            nodes[cursor + 1] = Node(_flattenedFrontier[cursorFrontier], 0, 0, 0);
            nodes[cursor + 2] = Node(_flattenedFrontier[cursorFrontier + 1], 0, 0, 0);

            // Move forward
            absolutePosition /= 3;
            localPosition = uint8(absolutePosition % 3);

            cursor += 3;
            cursorFrontier += 2;
        }

        // Add the root node
        nodes[cursor] = _createHoleNode(cursor, Position(localPosition));
        return cursor;
    }

    /// @dev Compute the index of the next node when going down in the tree.
    /// @param nodes The list of nodes of the tree
    /// @param nodeIndex The index of the starting node
    /// @param pos The position for going down, i.e. LEFT, MIDDLE or RIGHT.
    /// @return The index of the next node
    function _nextNodeIndex(
        Node[] memory nodes,
        uint64 nodeIndex,
        Position pos
    ) private pure returns (uint64) {
        if (pos == Position.LEFT) {
            return nodes[nodeIndex].left;
        } else if (pos == Position.MIDDLE) {
            return nodes[nodeIndex].middle;
        } else if (pos == Position.RIGHT) {
            return nodes[nodeIndex].right;
        } else {
            revert("unreachable");
        }
    }

    /// @dev Update the child of a node based on the position (which child to select) and an index to the new child.
    /// @param node node for which we want to update the child
    /// @param newChildIndex index of the new child
    /// @param pos position of the child node relative to the node (i.e. LEFT, MIDDLE or RIGHT)
    function _updateChildNode(
        Node memory node,
        uint64 newChildIndex,
        Position pos
    ) private pure {
        // Update the node
        if (pos == Position.LEFT) {
            node.left = newChildIndex;
        } else if (pos == Position.MIDDLE) {
            node.middle = newChildIndex;
        } else if (pos == Position.RIGHT) {
            node.right = newChildIndex;
        }
    }

    function _computeNodePos(uint64 absolutePos, uint64 branchIndex)
        private
        view
        returns (uint64, uint64)
    {
        uint64 localPos;
        uint64 divisor = uint64(3**(_merkleTreeHeight - branchIndex - 1));

        localPos = absolutePos / divisor;
        absolutePos = absolutePos % divisor;

        return (absolutePos, localPos);
    }

    /// @notice Insert an element into the tree in the position num_leaves.
    /// @param nodes The array of nodes
    /// @param rootIndex The index of the root node
    /// @param maxIndex The index of the latest element inserted in the nodes array
    /// @param element The value of the element to insert into the tree
    /// @return updated the value of maxIndex
    function _pushElement(
        Node[] memory nodes,
        uint64 rootIndex,
        uint64 maxIndex,
        uint256 element
    ) private returns (uint64) {
        require(_numLeaves < 3**_merkleTreeHeight, "The tree is full.");

        // Get the position of the leaf from the smart contract state
        uint64 leafPos = _numLeaves;
        uint64 branchIndex = 0;
        uint64 currentNodeIndex = rootIndex;
        uint64 previousNodeIndex = rootIndex;

        // Go down inside the tree until finding the first terminal node.
        uint64 absolutePos = leafPos;
        uint64 localPos = 0;
        while (!_isNull(nodes[currentNodeIndex])) {
            (absolutePos, localPos) = _computeNodePos(absolutePos, branchIndex);

            previousNodeIndex = currentNodeIndex;
            currentNodeIndex = _nextNodeIndex(nodes, currentNodeIndex, Position(localPos));

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

        while (branchIndex < _merkleTreeHeight - 1) {
            nodes[newNodeIndex] = Node(0, 0, 0, 0);
            _updateChildNode(nodes[previousNodeIndex], newNodeIndex, Position(localPos));

            // Prepare the next iteration of the loop
            previousNodeIndex = newNodeIndex;
            newNodeIndex += 1;
            branchIndex += 1;
            (absolutePos, localPos) = _computeNodePos(absolutePos, branchIndex);
        }

        // The last node contains the leaf value (compute the hash)
        // Remember position is computed with the remainder

        // Leaf node where the value is hash(0,_numLeaves,element)
        uint256 val = RescueLib.hash(0, _numLeaves, element);
        nodes[newNodeIndex] = Node(val, 0, 0, 0);
        _updateChildNode(nodes[previousNodeIndex], newNodeIndex, Position(localPos));

        // Increment the number of leaves
        //
        // This operation is costly and happens in a loop. However, for now the
        // merkle tree is usually updated with a single new element. In this
        // case we would not save gas by moving the update of _numLeaves. The
        // gas cost is also likely negligible compared to the whole operation of
        // inserting an element.
        //
        // slither-disable-next-line costly-loop
        _numLeaves += 1;

        // Return the new value of maxIndex
        return newNodeIndex;
    }

    /// @dev Store the frontier.
    /// @param nodes The list of node of the tree
    /// @param rootIndex The index of the root node
    function _storeFrontier(Node[] memory nodes, uint64 rootIndex) private {
        uint64 frontierSize = 2 * _merkleTreeHeight + 1;

        /// Collect the values from the root to the leaf but in reverse order
        uint64 currentNodeIndex = rootIndex;
        uint64 firstSiblingIndex = 0;
        uint64 secondSiblingIndex = 0;
        // Go down until the leaf
        for (uint256 i = 0; i < _merkleTreeHeight; i++) {
            // Pick the non-empty node that is most right
            Node memory currentNode = nodes[currentNodeIndex];
            if (!_isNull(nodes[currentNode.right])) {
                // Keep to the right
                currentNodeIndex = currentNode.right;
                firstSiblingIndex = currentNode.left;
                secondSiblingIndex = currentNode.middle;
            } else if (!_isNull(nodes[currentNode.middle])) {
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
            _flattenedFrontier[secondSiblingPos] = nodes[secondSiblingIndex].val;
            _flattenedFrontier[firstSiblingPos] = nodes[firstSiblingIndex].val;
        }
        // currentNodeIndex points to the leaf
        _flattenedFrontier[0] = nodes[currentNodeIndex].val;
    }

    /// @dev Update the state of the record merkle tree by inserting new elements.
    /// @param elements The list of elements to be appended to the current merkle tree described by the frontier.
    function updateRecordsMerkleTree(uint256[] memory elements) external onlyOwner {
        // The total number of nodes is bounded by 3*height+1 + 3*N*height = 3*(N+1)*height + 1
        // where N is the number of new records
        uint256 numElements = elements.length;
        Node[] memory nodes = new Node[](3 * (numElements + 1) * _merkleTreeHeight + 2);

        /// Insert the new elements ///

        // maxIndex tracks the index of the last element inserted in the tree
        uint64 rootIndex = _buildTreeFromFrontier(nodes);
        uint64 maxIndex = rootIndex;
        for (uint32 i = 0; i < elements.length; i++) {
            maxIndex = _pushElement(nodes, rootIndex, maxIndex, elements[i]);
        }
        //// Compute the root hash value ////
        _rootValue = _computeRootValueAndUpdateTree(nodes, rootIndex);

        //// Store the frontier
        _storeFrontier(nodes, rootIndex);
    }

    /// @notice Returns the root value of the Merkle tree.
    function getRootValue() public view returns (uint256) {
        return _rootValue;
    }

    /// @notice Returns the height of the Merkle tree.
    function getHeight() public view returns (uint8) {
        return _merkleTreeHeight;
    }

    /// @notice Returns the number of leaves of the Merkle tree.
    function getNumLeaves() public view returns (uint64) {
        return _numLeaves;
    }

    /// @dev Update the tree by hashing the children of each node.
    /// @param nodes The tree. Note that the nodes are updated by this function.
    /// @param rootNodePos The index of the root node in the list of nodes.
    /// @return The value obtained at the root.
    function _computeRootValueAndUpdateTree(Node[] memory nodes, uint256 rootNodePos)
        private
        returns (uint256)
    {
        // If the root node has no children return its value
        Node memory rootNode = nodes[rootNodePos];
        if (_isTerminal(rootNode)) {
            return rootNode.val;
        } else {
            uint256 valLeft = _computeRootValueAndUpdateTree(nodes, rootNode.left);
            uint256 valMiddle = _computeRootValueAndUpdateTree(nodes, rootNode.middle);
            uint256 valRight = _computeRootValueAndUpdateTree(nodes, rootNode.right);

            nodes[rootNode.left].val = valLeft;
            nodes[rootNode.middle].val = valMiddle;
            nodes[rootNode.right].val = valRight;

            return RescueLib.hash(valLeft, valMiddle, valRight);
        }
    }
}
