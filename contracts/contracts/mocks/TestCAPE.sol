//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "../CAPE.sol";

contract TestCAPE is CAPE {
    constructor(
        uint8 merkleTreeHeight,
        uint64 nRoots,
        address verifierAddr
    ) CAPE(merkleTreeHeight, nRoots, verifierAddr) {}

    function getNumLeaves() public view returns (uint256) {
        return _numLeaves;
    }

    function setInitialRecordCommitments(uint256[] memory elements) public {
        require(_rootValue == 0, "Merkle tree is nonempty");
        _updateRecordsMerkleTree(elements);
        for (uint256 i = 0; i < _roots.length; ++i) {
            _roots[i] = _rootValue;
        }
    }

    function insertNullifier(uint256 nullifier) public {
        return _insertNullifier(nullifier);
    }

    function checkTransfer(TransferNote memory note) public pure {
        return _checkTransfer(note);
    }

    function checkBurn(BurnNote memory note) public view {
        return _checkBurn(note);
    }

    function containsRoot(uint256 root) public view returns (bool) {
        return _containsRoot(root);
    }

    function containsBurnPrefix(bytes memory extraProofBoundData) public view returns (bool) {
        return _containsBurnPrefix(extraProofBoundData);
    }

    function containsBurnDestination(bytes memory extraProofBoundData) public view returns (bool) {
        return _containsBurnDestination(extraProofBoundData);
    }

    function containsBurnRecord(BurnNote memory note) public view returns (bool) {
        return _containsBurnRecord(note);
    }

    function deriveRecordCommitment(RecordOpening memory ro) public view returns (uint256) {
        return _deriveRecordCommitment(ro);
    }

    function addRoot(uint256 root) public {
        return _addRoot(root);
    }

    function setHeight(uint64 newHeight) public {
        blockHeight = newHeight;
    }

    function computeMaxCommitments(CapeBlock memory newBlock) public pure returns (uint256) {
        return _computeMaxCommitments(newBlock);
    }

    function checkForeignAssetCode(uint256 assetDefinitionCode, address erc20Address) public view {
        _checkForeignAssetCode(assetDefinitionCode, erc20Address);
    }

    function checkDomesticAssetCode(uint256 assetDefinitionCode, uint256 internalAssetCode)
        public
        view
    {
        _checkDomesticAssetCode(assetDefinitionCode, internalAssetCode);
    }

    function computeAssetDescription(address erc20Address, address sponsor)
        public
        pure
        returns (bytes memory)
    {
        return _computeAssetDescription(erc20Address, sponsor);
    }
}
