//SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {BN254 as C} from "../libraries/BN254.sol";

contract TestBN254 {
    constructor() {}

    // solhint-disable-next-line func-name-mixedcase
    function P1() public pure returns (C.G1Point memory) {
        return C.P1();
    }

    // solhint-disable-next-line func-name-mixedcase
    function P2() public pure returns (C.G2Point memory) {
        return C.P2();
    }

    function isInfinity(C.G1Point memory point) public pure returns (bool) {
        return C.isInfinity(point);
    }

    function negate(C.G1Point memory p) public pure returns (C.G1Point memory r) {
        return C.negate(p);
    }

    function add(C.G1Point memory p1, C.G1Point memory p2) public view returns (C.G1Point memory) {
        return C.add(p1, p2);
    }

    function scalarMul(C.G1Point memory p, uint256 s) public view returns (C.G1Point memory r) {
        return C.scalarMul(p, s);
    }

    function invert(uint256 fr) public view returns (uint256 output) {
        return C.invert(fr);
    }

    function validateG1Point(C.G1Point memory point) public pure {
        C.validateG1Point(point);
    }

    function validateScalarField(uint256 fr) public pure {
        C.validateScalarField(fr);
    }

    function pairingProd2(
        C.G1Point memory a1,
        C.G2Point memory a2,
        C.G1Point memory b1,
        C.G2Point memory b2
    ) public view returns (bool) {
        return C.pairingProd2(a1, a2, b1, b2);
    }

    function fromLeBytesModOrder(bytes memory leBytes) public pure returns (uint256) {
        return C.fromLeBytesModOrder(leBytes);
    }

    function isYNegative(C.G1Point memory p) public pure returns (bool) {
        return C.isYNegative(p);
    }

    function powSmall(
        uint256 base,
        uint256 exponent,
        uint256 modulus
    ) public pure returns (uint256) {
        return C.powSmall(base, exponent, modulus);
    }

    function testMultiScalarMul(C.G1Point[] memory bases, uint256[] memory scalars)
        public
        view
        returns (C.G1Point memory)
    {
        return C.multiScalarMul(bases, scalars);
    }

    function g1Serialize(C.G1Point memory p) public pure returns (bytes memory res) {
        return C.g1Serialize(p);
    }
}
