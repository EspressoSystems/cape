// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../interfaces/IPlonkVerifier.sol";
import "./Transfer2In2Out24DepthVk.sol";
import "./Transfer2In3Out24DepthVk.sol";
import "./Mint1In2Out24DepthVk.sol";
import "./Freeze3In3Out24DepthVk.sol";

library VerifyingKeys {
    function getVkById(uint256 encodedId)
        internal
        pure
        returns (IPlonkVerifier.VerifyingKey memory)
    {
        if (encodedId == getEncodedId(0, 2, 2, 24)) {
            // transfer/burn-2-input-2-output-24-depth
            return Transfer2In2Out24DepthVk.getVk();
        } else if (encodedId == getEncodedId(0, 2, 3, 24)) {
            // transfer/burn-2-input-3-output-24-depth
            return Transfer2In3Out24DepthVk.getVk();
        } else if (encodedId == getEncodedId(1, 1, 2, 24)) {
            // mint-1-input-2-output-24-depth
            return Mint1In2Out24DepthVk.getVk();
        } else if (encodedId == getEncodedId(2, 3, 3, 24)) {
            // freeze-3-input-3-output-24-depth
            return Freeze3In3Out24DepthVk.getVk();
        } else {
            revert("Unknown vk ID");
        }
    }

    // returns (noteType, numInput, numOutput, treeDepth) as a 4*8 = 32 byte = uint256
    // as the encoded ID.
    function getEncodedId(
        uint8 noteType,
        uint8 numInput,
        uint8 numOutput,
        uint8 treeDepth
    ) internal pure returns (uint256 encodedId) {
        assembly {
            encodedId := add(
                shl(24, noteType),
                add(shl(16, numInput), add(shl(8, numOutput), treeDepth))
            )
        }
    }
}
