// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import {VerifyingKeys as Vk} from "../libraries/VerifyingKeys.sol";
import "../interfaces/IPlonkVerifier.sol";

contract TestVerifyingKeys {
    function getVkById(uint256 encodedId)
        public
        pure
        returns (IPlonkVerifier.VerifyingKey memory)
    {
        return Vk.getVkById(encodedId);
    }

    function getEncodedId(
        uint8 noteType,
        uint8 numInput,
        uint8 numOutput,
        uint8 treeDepth
    ) public pure returns (uint256 encodedId) {
        return Vk.getEncodedId(noteType, numInput, numOutput, treeDepth);
    }
}
