//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {VerifyingKeys as Vk} from "../libraries/VerifyingKeys.sol";

contract TestVerifyingKeys {
    function getEncodedId(
        uint8 noteType,
        uint8 numInput,
        uint8 numOutput,
        uint8 treeDepth
    ) public pure returns (uint256 encodedId) {
        return Vk.getEncodedId(noteType, numInput, numOutput, treeDepth);
    }
}
