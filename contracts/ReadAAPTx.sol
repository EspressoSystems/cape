//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract ReadCAPTx {
    uint256 public scratch;

    constructor() {}

    struct TransferValidityProof {
        uint256 dummy;
    }

    struct GroupProjective {
        uint256 x;
        uint256 y;
        uint256 t;
        uint256 z;
    }

    struct EncKey {
        GroupProjective key;
    }

    struct AuditMemo {
        // is Ciphertext
        EncKey ephemeral;
        uint256[] data;
    }

    struct AuxInfo {
        uint256 merkleRoot;
        uint256 fee;
        uint256 validUntil;
        GroupProjective txnMemoVerKey;
    }

    struct TransferNote {
        uint256[] inputNullifiers;
        uint256[] outputCommitments;
        TransferValidityProof proof;
        AuditMemo auditMemo;
        AuxInfo auxInfo;
    }

    function readInt256(int256 x) public view returns (int256) {
        return x;
    }

    function addBlsScalar(uint256 x, uint256 y) public view returns (uint256) {
        return x + y;
    }

    function submitNullifiers(uint256[] calldata inputsNullifiers)
        public
        view
        returns (uint256)
    {
        return inputsNullifiers.length;
    }

    function submitTransferNote(TransferNote calldata note) public {
        scratch = note.inputNullifiers[0];
    }
}
