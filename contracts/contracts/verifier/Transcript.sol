//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "solidity-bytes-utils/contracts/BytesLib.sol";
import "hardhat/console.sol";
import "../libraries/Utils.sol";
import {BN254} from "../libraries/BN254.sol";
import {IPlonkVerifier} from "../interfaces/IPlonkVerifier.sol";

library Transcript {
    struct TranscriptData {
        bytes transcript;
        bytes32[2] state;
    }

    // ================================
    // Primitive functions
    // ================================
    function appendMessage(TranscriptData memory self, bytes memory message) internal pure {
        self.transcript = abi.encodePacked(self.transcript, message);
    }

    function appendFieldElement(TranscriptData memory self, uint256 fieldElement) internal pure {
        appendMessage(self, abi.encodePacked(Utils.reverseEndianness(fieldElement)));
    }

    function appendGroupElement(TranscriptData memory self, BN254.G1Point memory comm)
        internal
        pure
    {
        bytes memory commBytes = BN254.g1Serialize(comm);
        appendMessage(self, commBytes);
    }

    // ================================
    // Transcript APIs
    // ================================
    function appendChallenge(TranscriptData memory self, uint256 challenge) internal pure {
        appendFieldElement(self, challenge);
    }

    function appendCommitments(TranscriptData memory self, BN254.G1Point[] memory comms)
        internal
        pure
    {
        for (uint256 i = 0; i < comms.length; i++) {
            appendCommitment(self, comms[i]);
        }
    }

    function appendCommitment(TranscriptData memory self, BN254.G1Point memory comm)
        internal
        pure
    {
        appendGroupElement(self, comm);
    }

    function getAndAppendChallenge(TranscriptData memory self) internal pure returns (uint256) {
        bytes32 h1 = keccak256(
            abi.encodePacked(self.state[0], self.state[1], self.transcript, uint8(0))
        );
        bytes32 h2 = keccak256(
            abi.encodePacked(self.state[0], self.state[1], self.transcript, uint8(1))
        );

        self.state[0] = h1;
        self.state[1] = h2;

        return BN254.fromLeBytesModOrder(BytesLib.slice(abi.encodePacked(h1, h2), 0, 48));
    }

    /// @dev Append the verifying key and the public inputs to the transcript.
    /// @param verifyingKey verifiying key
    /// @param publicInput a list of field elements
    function appendVkAndPubInput(
        TranscriptData memory self,
        IPlonkVerifier.VerifyingKey memory verifyingKey,
        uint256[] memory publicInput
    ) internal pure {
        uint64 sizeInBits = 254;

        // Fr field size in bits
        appendMessage(
            self,
            BytesLib.slice(abi.encodePacked(Utils.reverseEndianness(sizeInBits)), 0, 8)
        );

        // domain size
        appendMessage(
            self,
            BytesLib.slice(
                abi.encodePacked(Utils.reverseEndianness(verifyingKey.domainSize)),
                0,
                8
            )
        );

        // number of inputs
        appendMessage(
            self,
            BytesLib.slice(abi.encodePacked(Utils.reverseEndianness(verifyingKey.numInputs)), 0, 8)
        );

        // =====================
        // k: coset representatives
        // =====================
        // Currently, K is hardcoded, and there are 5 of them since
        // # wire types == 5
        appendFieldElement(self, 0x1); // k0 = 1
        appendFieldElement(
            self,
            0x2f8dd1f1a7583c42c4e12a44e110404c73ca6c94813f85835da4fb7bb1301d4a
        ); // k1
        appendFieldElement(
            self,
            0x1ee678a0470a75a6eaa8fe837060498ba828a3703b311d0f77f010424afeb025
        ); // k2
        appendFieldElement(
            self,
            0x2042a587a90c187b0a087c03e29c968b950b1db26d5c82d666905a6895790c0a
        ); // k3
        appendFieldElement(
            self,
            0x2e2b91456103698adf57b799969dea1c8f739da5d8d40dd3eb9222db7c81e881
        ); // k4

        // selectors
        appendGroupElement(self, verifyingKey.q1);
        appendGroupElement(self, verifyingKey.q2);
        appendGroupElement(self, verifyingKey.q3);
        appendGroupElement(self, verifyingKey.q4);
        appendGroupElement(self, verifyingKey.qM12);
        appendGroupElement(self, verifyingKey.qM34);
        appendGroupElement(self, verifyingKey.qH1);
        appendGroupElement(self, verifyingKey.qH2);
        appendGroupElement(self, verifyingKey.qH3);
        appendGroupElement(self, verifyingKey.qH4);
        appendGroupElement(self, verifyingKey.qO);
        appendGroupElement(self, verifyingKey.qC);
        appendGroupElement(self, verifyingKey.qEcc);

        // sigmas
        appendGroupElement(self, verifyingKey.sigma0);
        appendGroupElement(self, verifyingKey.sigma1);
        appendGroupElement(self, verifyingKey.sigma2);
        appendGroupElement(self, verifyingKey.sigma3);
        appendGroupElement(self, verifyingKey.sigma4);

        // public inputs
        for (uint256 i = 0; i < publicInput.length; i++) {
            appendFieldElement(self, publicInput[i]);
        }
    }

    /// @dev Append the proof to the transcript.
    function appendProofEvaluations(
        TranscriptData memory self,
        IPlonkVerifier.PlonkProof memory proof
    ) internal pure {
        appendFieldElement(self, proof.wireEval0);
        appendFieldElement(self, proof.wireEval1);
        appendFieldElement(self, proof.wireEval2);
        appendFieldElement(self, proof.wireEval3);
        appendFieldElement(self, proof.wireEval4);

        appendFieldElement(self, proof.sigmaEval0);
        appendFieldElement(self, proof.sigmaEval1);
        appendFieldElement(self, proof.sigmaEval2);
        appendFieldElement(self, proof.sigmaEval3);

        appendFieldElement(self, proof.prodPermZetaOmegaEval);
    }
}
