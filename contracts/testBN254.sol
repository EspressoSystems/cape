//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.7.0;
pragma experimental ABIEncoderV2;

import { Curve as C } from "./BN254.sol";


contract testBN254  {

    using C for *;

    constructor() public {}

    // TODO can we avoid duplicting C. everywhere?
    function g1add(C.G1Point memory p1, C.G1Point memory p2) public returns (C.G1Point memory r) {
        return C.g1add(p1, p2);
    }

    function g1mul(C.G1Point memory p1, uint s) public returns (C.G1Point memory r) {
        return C.g1mul(p1, s);
    }
}
