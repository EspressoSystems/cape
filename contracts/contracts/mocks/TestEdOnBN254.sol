//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {EdOnBN254 as C} from "../libraries/EdOnBN254.sol";

contract TestEdOnBN254 {
    constructor() {}

    function edSerialize(C.EdOnBN254Point memory p) public pure returns (bytes memory res) {
        return C.edSerialize(p);
    }

    function checkEdOnBn254Point(C.EdOnBN254Point memory p)
        public
        pure
        returns (C.EdOnBN254Point memory)
    {
        return p;
    }
}
