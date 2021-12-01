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

    // XXX This wrapper around the solidity array type is to workaround
    // an issue that causes the ethers abigen to fail on nested structs.
    //     https://github.com/gakonst/ethers-rs/issues/538
    struct Array {
        uint256[] items;
    }

    struct AuditMemo {
        // is Ciphertext
        EncKey ephemeral;
        Array data;
    }

    struct AuxInfo {
        uint256 merkleRoot;
        uint256 fee;
        uint256 validUntil;
        GroupProjective txnMemoVerKey;
    }

    struct TransferNote {
        Array inputNullifiers;
        Array outputCommitments;
        TransferValidityProof proof;
        AuditMemo auditMemo;
        AuxInfo auxInfo;
    }

    function submitTransferNote(TransferNote calldata note) public {
        scratch = note.inputNullifiers.items[0];
    }
}
