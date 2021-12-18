//SPDX-License-Identifier: MIT
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

    function checkAssetCode(uint256 code) public pure returns (uint256) {
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

    function checkNoteType(CAPE.NoteType t)
        public
        pure
        returns (CAPE.NoteType)
    {
        return t;
    }

    function checkEdOnBn254Point(CAPE.EdOnBn254Point memory p)
        public
        pure
        returns (CAPE.EdOnBn254Point memory)
    {
        return p;
    }

    function checkMintNote(CAPE.MintNote memory note)
        public
        pure
        returns (CAPE.MintNote memory)
    {
        return note;
    }

    function checkFreezeNote(CAPE.FreezeNote memory note)
        public
        pure
        returns (CAPE.FreezeNote memory)
    {
        return note;
    }

    function checkBurnNote(CAPE.BurnNote memory note)
        public
        pure
        returns (CAPE.BurnNote memory)
    {
        return note;
    }

    function checkTransferNote(CAPE.TransferNote memory note)
        public
        pure
        returns (CAPE.TransferNote memory)
    {
        return note;
    }

    function checkCapeBlock(CAPE.CapeBlock memory b)
        public
        pure
        returns (CAPE.CapeBlock memory)
    {
        return b;
    }
}
