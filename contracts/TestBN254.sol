//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Curve as C} from "./BN254.sol";

contract TestBN254 {
    constructor() public {}

    // TODO can we avoid duplicating C. everywhere?
    function g1Add(C.G1Point memory p1, C.G1Point memory p2)
        public
        returns (C.G1Point memory r)
    {
        return C.g1add(p1, p2);
    }

    function g1Mul(C.G1Point memory p1, uint256 s)
        public
        returns (C.G1Point memory r)
    {
        return C.g1mul(p1, s);
    }

    function pairingCheck(C.G1Point[] memory p1, C.G2Point[] memory p2)
        public
        returns (bool)
    {
        return C.pairing(p1, p2);
    }
}
