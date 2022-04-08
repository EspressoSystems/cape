// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import "../interfaces/IPlonkVerifier.sol";
import "./Transfer1In2Out24DepthVk.sol";
import "./Transfer2In2Out24DepthVk.sol";
import "./Transfer2In3Out24DepthVk.sol";
import "./Mint1In2Out24DepthVk.sol";
import "./Freeze2In2Out24DepthVk.sol";
import "./Freeze3In3Out24DepthVk.sol";

library VerifyingKeys {
    function getVkById(uint256 encodedId)
        external
        pure
        returns (IPlonkVerifier.VerifyingKey memory)
    {
        if (encodedId == getEncodedId(0, 1, 2, 24)) {
            // transfer/burn-1-input-2-output-24-depth
            return Transfer1In2Out24DepthVk.getVk();
        } else if (encodedId == getEncodedId(0, 2, 2, 24)) {
            // transfer/burn-2-input-2-output-24-depth
            return Transfer2In2Out24DepthVk.getVk();
        } else if (encodedId == getEncodedId(0, 2, 3, 24)) {
            // transfer/burn-2-input-3-output-24-depth
            return Transfer2In3Out24DepthVk.getVk();
        } else if (encodedId == getEncodedId(1, 1, 2, 24)) {
            // mint-1-input-2-output-24-depth
            return Mint1In2Out24DepthVk.getVk();
        } else if (encodedId == getEncodedId(2, 2, 2, 24)) {
            // freeze-2-input-2-output-24-depth
            return Freeze2In2Out24DepthVk.getVk();
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
    ) public pure returns (uint256 encodedId) {
        assembly {
            encodedId := add(
                shl(24, noteType),
                add(shl(16, numInput), add(shl(8, numOutput), treeDepth))
            )
        }
    }
}
