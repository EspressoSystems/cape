// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import {EdOnBN254 as C} from "../libraries/EdOnBN254.sol";

contract TestEdOnBN254 {
    constructor() {}

    function serialize(C.EdOnBN254Point memory p) public pure returns (bytes memory res) {
        return C.serialize(p);
    }

    function checkEdOnBn254Point(C.EdOnBN254Point memory p)
        public
        pure
        returns (C.EdOnBN254Point memory)
    {
        return p;
    }
}
