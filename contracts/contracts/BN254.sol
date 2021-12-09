// SPDX-License-Identifier: MIT
//
// Based on : https://gist.githubusercontent.com/chriseth/f9be9d9391efc5beb9704255a8e2989d/raw/4d0fb90847df1d4e04d507019031888df8372239/snarktest.solidity
// Copyright 2017 Christian Reitwiessner
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

pragma solidity ^0.8.0;

/// @notice Barreto-Naehrig curve over a 254 bit prime field
library Curve {
    // use notation from https://datatracker.ietf.org/doc/draft-irtf-cfrg-pairing-friendly-curves/
    //
    // Elliptic curve is defined over a prime field GF(p), with embedding degree k.
    // Short Weierstrass (SW form) is, for a, b \in GF(p^n) for some natural number n > 0:
    //   E: y^2 = x^3 + a * x + b
    //
    // Pairing is defined over cyclic subgroups G1, G2, both of which are of order r.
    // G1 is a subgroup of E(GF(p)), G2 is a subgroup of E(GF(p^k)).
    //
    // BN family are parameterized curves with well-chosen t,
    //   p = 36 * t^4 + 36 * t^3 + 24 * t^2 + 6 * t + 1
    //   r = 36 * t^4 + 36 * t^3 + 18 * t^2 + 6 * t + 1
    // for some integer t.
    // E has the equation:
    //   E: y^2 = x^3 + b
    // where b is a primitive element of multiplicative group (GF(p))^* of order (p-1).
    // A pairing e is defined by taking G1 as a subgroup of E(GF(p)) of order r,
    // G2 as a subgroup of E'(GF(p^2)),
    // and G_T as a subgroup of a multiplicative group (GF(p^12))^* of order r.
    //
    // BN254 is defined over a 254-bit prime order p, embedding degree k = 12.
    uint256 public constant P_MOD =
        21888242871839275222246405745257275088696311157297823662689037894645226208583;
    uint256 public constant R_MOD =
        21888242871839275222246405745257275088548364400416034343698204186575808495617;

    struct G1Point {
        uint256 x;
        uint256 y;
    }

    // G2 group element where x \in Fp2 = x0 * z + x1
    struct G2Point {
        uint256 x0;
        uint256 x1;
        uint256 y0;
        uint256 y1;
    }

    /// @return the generator of G1
    // solhint-disable-next-line func-name-mixedcase
    function P1() internal pure returns (G1Point memory) {
        return G1Point(1, 2);
    }

    /// @return the generator of G2
    // solhint-disable-next-line func-name-mixedcase
    function P2() internal pure returns (G2Point memory) {
        return
            G2Point({
                x0: 0x198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2,
                x1: 0x1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed,
                y0: 0x090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b,
                y1: 0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa
            });
    }

    /// @dev check if a G1 point is Infinity
    /// @notice precompile bn256Add at address(6) takes (0, 0) as Point of Infinity,
    /// some crypto libraries (such as arkwork) uses a boolean flag to mark PoI, and
    /// just use (0, 1) as affine coordinates (not on curve) to represents PoI.
    function isInfinity(G1Point memory point) internal pure returns (bool) {
        bool result;
        assembly {
            let x := mload(point)
            let y := mload(add(point, 0x20))
            result := and(iszero(x), iszero(y))
        }
        return result;
    }

    /// @return r the negation of p, i.e. p.add(p.negate()) should be zero.
    function negate(G1Point memory p) internal pure returns (G1Point memory r) {
        if (isInfinity(p)) return p;
        return G1Point(p.x, P_MOD - (p.y % P_MOD));
    }

    /// @return r the sum of two points of G1
    function add(G1Point memory p1, G1Point memory p2)
        internal
        returns (G1Point memory r)
    {
        uint256[4] memory input;
        input[0] = p1.x;
        input[1] = p1.y;
        input[2] = p2.x;
        input[3] = p2.y;
        bool success;
        assembly {
            success := call(sub(gas(), 2000), 6, 0, input, 0xc0, r, 0x60)
            // Use "invalid" to make gas estimation work
            switch success
            case 0 {
                revert(0, 0)
            }
        }
        require(success, "Bn254: group addition failed!");
    }

    /// @return r the product of a point on G1 and a scalar, i.e.
    /// p == p.mul(1) and p.add(p) == p.mul(2) for all points p.
    function scalarMul(G1Point memory p, uint256 s)
        internal
        view
        returns (G1Point memory r)
    {
        uint256[3] memory input;
        input[0] = p.x;
        input[1] = p.y;
        input[2] = s;
        bool success;
        assembly {
            success := staticcall(sub(gas(), 2000), 7, input, 0x80, r, 0x60)
            // Use "invalid" to make gas estimation work
            switch success
            case 0 {
                revert(0, 0)
            }
        }
        require(success, "Bn254: scalar mul failed!");
    }

    /// @dev Compute f^-1 for f \in Fr scalar field
    /// @notice credit: Aztec, Spilsbury Holdings Ltd
    function invert(uint256 fr) internal view returns (uint256 output) {
        bool success;
        uint256 p = R_MOD;
        assembly {
            let mPtr := mload(0x40)
            mstore(mPtr, 0x20)
            mstore(add(mPtr, 0x20), 0x20)
            mstore(add(mPtr, 0x40), 0x20)
            mstore(add(mPtr, 0x60), fr)
            mstore(add(mPtr, 0x80), sub(p, 2))
            mstore(add(mPtr, 0xa0), p)
            success := staticcall(gas(), 0x05, mPtr, 0xc0, 0x00, 0x20)
            output := mload(0x00)
        }
        require(success, "Bn254: pow precompile failed!");
        return output;
    }

    /**
     * validate the following:
     *   x != 0
     *   y != 0
     *   x < p
     *   y < p
     *   y^2 = x^3 + 3 mod p
     */
    /// @dev validate G1 point and check if it is on curve
    /// @notice credit: Aztec, Spilsbury Holdings Ltd
    function validateG1Point(G1Point memory point) internal pure {
        bool isWellFormed;
        uint256 p = P_MOD;
        assembly {
            let x := mload(point)
            let y := mload(add(point, 0x20))

            isWellFormed := and(
                and(and(lt(x, p), lt(y, p)), not(or(iszero(x), iszero(y)))),
                eq(mulmod(y, y, p), addmod(mulmod(x, mulmod(x, x, p), p), 3, p))
            )
        }
        require(isWellFormed, "Bn254: invalid G1 point");
    }

    /// @dev Evaluate the following pairing product:
    /// @dev e(a1, a2).e(-b1, b2) == 1
    /// @notice credit: Aztec, Spilsbury Holdings Ltd
    function pairingProd2(
        G1Point memory a1,
        G2Point memory a2,
        G1Point memory b1,
        G2Point memory b2
    ) internal view returns (bool success) {
        validateG1Point(a1);
        validateG1Point(b1);
        uint256 out;
        assembly {
            let mPtr := mload(0x40)
            mstore(mPtr, mload(a1))
            mstore(add(mPtr, 0x20), mload(add(a1, 0x20)))
            mstore(add(mPtr, 0x40), mload(a2))
            mstore(add(mPtr, 0x60), mload(add(a2, 0x20)))
            mstore(add(mPtr, 0x80), mload(add(a2, 0x40)))
            mstore(add(mPtr, 0xa0), mload(add(a2, 0x60)))

            mstore(add(mPtr, 0xc0), mload(b1))
            mstore(add(mPtr, 0xe0), mload(add(b1, 0x20)))
            mstore(add(mPtr, 0x100), mload(b2))
            mstore(add(mPtr, 0x120), mload(add(b2, 0x20)))
            mstore(add(mPtr, 0x140), mload(add(b2, 0x40)))
            mstore(add(mPtr, 0x160), mload(add(b2, 0x60)))
            success := staticcall(gas(), 8, mPtr, 0x180, 0x00, 0x20)
            out := mload(0x00)
        }
        require(success, "Bn254: Pairing check failed!");
        return (out != 0);
    }

    function fromLeBytesModOrder(bytes memory leBytes)
        internal
        pure
        returns (uint256 ret)
    {
        // TODO: Can likely be gas optimized by copying the first 31 bytes directly.
        for (uint256 i = 0; i < leBytes.length; i++) {
            ret = mulmod(ret, 256, R_MOD);
            ret = addmod(
                ret,
                uint256(uint8(leBytes[leBytes.length - 1 - i])),
                R_MOD
            );
        }
    }

    /// @dev Check if y-coordinate of G1 point is negative.
    function isYNegative(G1Point memory point) internal pure returns (bool) {
        return point.y < P_MOD / 2;
    }

    // @dev Perform a modular exponentiation.
    // @return base^exponent (mod modulus)
    // This method is ideal for small exponents (~64 bits or less), as it is cheaper than using the pow precompile
    // @notice credit: credit: Aztec, Spilsbury Holdings Ltd
    function powSmall(
        uint256 base,
        uint256 exponent,
        uint256 modulus
    ) internal pure returns (uint256) {
        uint256 result = 1;
        uint256 input = base;
        uint256 count = 1;

        assembly {
            let endpoint := add(exponent, 0x01)
            for {

            } lt(count, endpoint) {
                count := add(count, count)
            } {
                if and(exponent, count) {
                    result := mulmod(result, input, modulus)
                }
                input := mulmod(input, input, modulus)
            }
        }

        return result;
    }
}
