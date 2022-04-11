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
import "../interfaces/IPlonkVerifier.sol";

contract TestCapeTypes {
    function checkNullifier(uint256 nf) public pure returns (uint256) {
        return nf;
    }

    function checkRecordCommitment(uint256 rc) public pure returns (uint256) {
        return rc;
    }

    function checkMerkleRoot(uint256 root) public pure returns (uint256) {
        return root;
    }

    function checkForeignAssetCode(uint256 code) public pure returns (uint256) {
        return code;
    }

    function checkAssetPolicy(CAPE.AssetPolicy memory policy)
        public
        pure
        returns (CAPE.AssetPolicy memory)
    {
        return policy;
    }

    function checkAssetDefinition(CAPE.AssetDefinition memory def)
        public
        pure
        returns (CAPE.AssetDefinition memory)
    {
        return def;
    }

    function checkRecordOpening(CAPE.RecordOpening memory ro)
        public
        pure
        returns (CAPE.RecordOpening memory)
    {
        return ro;
    }

    function checkPlonkProof(IPlonkVerifier.PlonkProof memory proof)
        public
        pure
        returns (IPlonkVerifier.PlonkProof memory)
    {
        return proof;
    }

    function checkAuditMemo(CAPE.AuditMemo memory memo)
        public
        pure
        returns (CAPE.AuditMemo memory)
    {
        return memo;
    }

    function checkNoteType(CAPE.NoteType t) public pure returns (CAPE.NoteType) {
        return t;
    }

    function checkMintNote(CAPE.MintNote memory note) public pure returns (CAPE.MintNote memory) {
        return note;
    }

    function checkFreezeNote(CAPE.FreezeNote memory note)
        public
        pure
        returns (CAPE.FreezeNote memory)
    {
        return note;
    }

    function checkBurnNote(CAPE.BurnNote memory note) public pure returns (CAPE.BurnNote memory) {
        return note;
    }

    function checkTransferNote(CAPE.TransferNote memory note)
        public
        pure
        returns (CAPE.TransferNote memory)
    {
        return note;
    }

    function checkCapeBlock(CAPE.CapeBlock memory b) public pure returns (CAPE.CapeBlock memory) {
        return b;
    }
}
