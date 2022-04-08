// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import "hardhat/console.sol";

import {BN254} from "../libraries/BN254.sol";
import {Transcript} from "../verifier/Transcript.sol";
import {IPlonkVerifier} from "../interfaces/IPlonkVerifier.sol";

contract TestTranscript {
    using Transcript for Transcript.TranscriptData;

    function appendMessage(Transcript.TranscriptData memory transcript, bytes memory message)
        public
        pure
        returns (Transcript.TranscriptData memory)
    {
        transcript.appendMessage(message);
        return transcript;
    }

    function appendChallenge(Transcript.TranscriptData memory transcript, uint256 challenge)
        public
        pure
        returns (Transcript.TranscriptData memory)
    {
        transcript.appendChallenge(challenge);
        return transcript;
    }

    function getAndAppendChallenge(Transcript.TranscriptData memory transcript)
        public
        pure
        returns (uint256)
    {
        return transcript.getAndAppendChallenge();
    }

    function testAppendMessageAndGet(
        Transcript.TranscriptData memory transcript,
        bytes memory message
    ) public pure returns (uint256) {
        transcript.appendMessage(message);
        return transcript.getAndAppendChallenge();
    }

    function testAppendChallengeAndGet(
        Transcript.TranscriptData memory transcript,
        uint256 challenge
    ) public pure returns (uint256) {
        transcript.appendChallenge(challenge);
        return transcript.getAndAppendChallenge();
    }

    function testAppendCommitmentAndGet(
        Transcript.TranscriptData memory transcript,
        BN254.G1Point memory comm
    ) public pure returns (uint256) {
        transcript.appendCommitment(comm);
        return transcript.getAndAppendChallenge();
    }

    function testAppendCommitmentsAndGet(
        Transcript.TranscriptData memory transcript,
        BN254.G1Point[] memory comms
    ) public pure returns (uint256) {
        transcript.appendCommitments(comms);
        return transcript.getAndAppendChallenge();
    }

    function testGetAndAppendChallengeMultipleTimes(
        Transcript.TranscriptData memory transcript,
        uint256 times
    ) public pure returns (uint256 challenge) {
        for (uint256 i = 0; i < times; i++) {
            challenge = transcript.getAndAppendChallenge();
        }
    }

    function testAppendVkAndPubInput(
        Transcript.TranscriptData memory transcript,
        IPlonkVerifier.VerifyingKey memory verifyingKey,
        uint256[] memory pubInputs
    ) public pure returns (Transcript.TranscriptData memory) {
        transcript.appendVkAndPubInput(verifyingKey, pubInputs);
        return transcript;
    }

    function testAppendProofEvaluations(
        Transcript.TranscriptData memory transcript,
        IPlonkVerifier.PlonkProof memory proof
    ) public pure returns (Transcript.TranscriptData memory) {
        transcript.appendProofEvaluations(proof);
        return transcript;
    }
}
