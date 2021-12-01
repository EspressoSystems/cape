//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "solidity-bytes-utils/contracts/BytesLib.sol";
import "hardhat/console.sol";
import {Curve} from "./BN254.sol";

library Transcript {
    struct TranscriptData {
        bytes transcript;
        bytes32[2] state;
    }

    function appendMessage(TranscriptData memory self, bytes memory message)
        internal
        pure
    {
        self.transcript = abi.encodePacked(self.transcript, message);
    }

    function reverseEndianness(uint256 input)
        internal
        pure
        returns (uint256 v)
    {
        v = input;

        // swap bytes
        v =
            ((v &
                0xFF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00) >>
                8) |
            ((v &
                0x00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF) <<
                8);

        // swap 2-byte long pairs
        v =
            ((v &
                0xFFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000) >>
                16) |
            ((v &
                0x0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF) <<
                16);

        // swap 4-byte long pairs
        v =
            ((v &
                0xFFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000) >>
                32) |
            ((v &
                0x00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF) <<
                32);

        // swap 8-byte long pairs
        v =
            ((v &
                0xFFFFFFFFFFFFFFFF0000000000000000FFFFFFFFFFFFFFFF0000000000000000) >>
                64) |
            ((v &
                0x0000000000000000FFFFFFFFFFFFFFFF0000000000000000FFFFFFFFFFFFFFFF) <<
                64);

        // swap 16-byte long pairs
        v = (v >> 128) | (v << 128);
    }

    function appendChallenge(TranscriptData memory self, uint256 challenge)
        internal
        pure
    {
        appendMessage(self, abi.encodePacked(reverseEndianness(challenge)));
    }

    function appendCommitments(
        TranscriptData memory self,
        Curve.G1Point[] memory comms
    ) internal pure {
        for (uint256 i = 0; i < comms.length; i++) {
            appendCommitment(self, comms[i]);
        }
    }

    function appendCommitment(
        TranscriptData memory self,
        Curve.G1Point memory comm
    ) internal pure {
        bytes memory commBytes = g1Serialize(comm);
        appendMessage(self, commBytes);
    }

    function g1Serialize(Curve.G1Point memory point)
        internal
        pure
        returns (bytes memory)
    {
        uint256 mask;

        // Set the 254-th bit to 1 for infinity
        // https://docs.rs/ark-serialize/0.3.0/src/ark_serialize/flags.rs.html#117
        if (Curve.isZero(point)) {
            mask |= 0x4000000000000000000000000000000000000000000000000000000000000000;
        }

        // Set the 255-th bit to 1 for negative Y
        // https://docs.rs/ark-serialize/0.3.0/src/ark_serialize/flags.rs.html#118
        if (Curve.isYNegative(point)) {
            mask = 0x8000000000000000000000000000000000000000000000000000000000000000;
        }

        return abi.encodePacked(reverseEndianness(point.X | mask));
    }

    function getAndAppendChallenge(TranscriptData memory self)
        internal
        pure
        returns (uint256)
    {
        bytes32 h1 = keccak256(
            abi.encodePacked(
                self.state[0],
                self.state[1],
                self.transcript,
                uint8(0)
            )
        );
        bytes32 h2 = keccak256(
            abi.encodePacked(
                self.state[0],
                self.state[1],
                self.transcript,
                uint8(1)
            )
        );

        self.state[0] = h1;
        self.state[1] = h2;

        bytes memory randomBytes = BytesLib.slice(
            abi.encodePacked(h1, h2),
            0,
            48
        );
        return Curve.fromLeBytesModOrder(randomBytes);
    }
}
