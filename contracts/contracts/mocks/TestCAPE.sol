// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

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
        addRoot(_rootValue);
    }

    function publish(uint256 nullifier) public {
        return _publish(nullifier);
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

    function containsBurnPrefix(bytes memory extraProofBoundData) public pure returns (bool) {
        return _containsBurnPrefix(extraProofBoundData);
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

    function computeNumCommitments(CapeBlock memory newBlock) public pure returns (uint256) {
        return _computeNumCommitments(newBlock);
    }

    function checkForeignAssetCode(
        uint256 assetDefinitionCode,
        address erc20Address,
        address sponsor,
        AssetPolicy memory policy
    ) public pure {
        _checkForeignAssetCode(assetDefinitionCode, erc20Address, sponsor, policy);
    }

    function checkDomesticAssetCode(uint256 assetDefinitionCode, uint256 internalAssetCode)
        public
        pure
    {
        _checkDomesticAssetCode(assetDefinitionCode, internalAssetCode);
    }

    function computeAssetDescription(
        address erc20Address,
        address sponsor,
        AssetPolicy memory policy
    ) public pure returns (bytes memory) {
        return _computeAssetDescription(erc20Address, sponsor, policy);
    }

    function pendingDepositsLength() public view returns (uint256) {
        return pendingDeposits.length;
    }

    function fillUpPendingDepositsQueue() public {
        for (uint256 i = pendingDeposits.length; i < MAX_NUM_PENDING_DEPOSIT; i++) {
            pendingDeposits.push(100 + i);
        }
    }
}
