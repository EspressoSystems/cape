//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "hardhat/console.sol";

// TODO: we have name collisions in src/bindings/mod.rs
// Consider not pulling everything to top-level
import {BN254} from "../libraries/BN254.sol";
import {Transcript} from "../verifier/Transcript.sol";

contract TestTranscript {
    using Transcript for Transcript.TranscriptData;

    function appendMessage(
        Transcript.TranscriptData memory transcript,
        bytes memory message
    ) public pure returns (Transcript.TranscriptData memory) {
        transcript.appendMessage(message);
        return transcript;
    }

    function appendChallenge(
        Transcript.TranscriptData memory transcript,
        uint256 challenge
    ) public pure returns (Transcript.TranscriptData memory) {
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
}
