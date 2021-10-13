//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract ReadAAPTx {
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
        uint256 merkle_root;
        uint256 fee;
        uint256 valid_until;
        GroupProjective txn_memo_ver_key;
    }

    struct TransferNote {
        uint256[] input_nullifiers;
        uint256[] output_commitments;
        TransferValidityProof proof;
        AuditMemo audit_memo;
        AuxInfo aux_info;
    }

    function readInt256(int256 x) public view returns (int256) {
        return x;
    }

    function addBlsScalar(uint256 x, uint256 y) public view returns (uint256) {
        return x + y;
    }

    function submitNullifiers(uint256[] calldata inputs_nullifiers)
        public
        view
        returns (uint256)
    {
        return inputs_nullifiers.length;
    }

    function submitTransferNote(TransferNote calldata note) public {
        scratch = note.input_nullifiers[0];
    }
}
