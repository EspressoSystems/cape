//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "../CAPE.sol";

contract TestCAPE is CAPE {
    constructor(uint8 height, uint64 nRoots) CAPE(height, nRoots) {}

    function testOnlyGetNumLeaves() public view returns (uint256) {
        return _numLeaves;
    }

    function testOnlyInsertRecordCommitments(uint256[] memory elements) public {
        _updateRecordsMerkleTree(elements);
        _roots[0] = _rootValue;
    }

    function insertNullifier(uint256 nullifier) public {
        return _insertNullifier(nullifier);
    }

    function checkTransfer(TransferNote memory note) public view {
        return _checkTransfer(note);
    }

    function checkBurn(BurnNote memory note) public view {
        return _checkBurn(note);
    }

    function containsBurnPrefix(bytes memory extraProofBoundData)
        public
        view
        returns (bool)
    {
        return _containsBurnPrefix(extraProofBoundData);
    }

    function containsBurnDestination(bytes memory extraProofBoundData)
        public
        view
        returns (bool)
    {
        return _containsBurnDestination(extraProofBoundData);
    }

    function containsBurnRecord(BurnNote memory note)
        public
        view
        returns (bool)
    {
        return _containsBurnRecord(note);
    }

    function deriveRecordCommitment(RecordOpening memory ro)
        public
        view
        returns (uint256)
    {
        return _deriveRecordCommitment(ro);
    }

    function addRoot(uint256 root) public {
        return _addRoot(root);
    }

    function computeMaxCommitments(CapeBlock memory newBlock)
        public
        pure
        returns (uint256)
    {
        return _computeMaxCommitments(newBlock);
    }
}
