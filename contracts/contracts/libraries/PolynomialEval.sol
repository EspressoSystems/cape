// SPDX-License-Identifier: GPL-3.0-or-later

// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import {BN254} from "../libraries/BN254.sol";

library PolynomialEval {
    /// @dev a Radix 2 Evaluation Domain
    struct EvalDomain {
        uint256 logSize; // log_2(self.size)
        uint256 size; // Size of the domain as a field element
        uint256 sizeInv; // Inverse of the size in the field
        uint256 groupGen; // A generator of the subgroup
        uint256 groupGenInv; // Inverse of the generator of the subgroup
    }

    /// @dev stores vanishing poly, lagrange at 1, and Public input poly
    struct EvalData {
        uint256 vanishEval;
        uint256 lagrangeOne;
        uint256 piEval;
    }

    /// @dev compute the EvalData for a given domain and a challenge zeta
    function evalDataGen(
        EvalDomain memory self,
        uint256 zeta,
        uint256[] memory publicInput
    ) internal view returns (EvalData memory evalData) {
        evalData.vanishEval = evaluateVanishingPoly(self, zeta);
        evalData.lagrangeOne = evaluateLagrangeOne(self, zeta, evalData.vanishEval);
        evalData.piEval = evaluatePiPoly(self, publicInput, zeta, evalData.vanishEval);
    }

    /// @dev Create a new Radix2EvalDomain with `domainSize` which should be power of 2.
    /// @dev Will revert if domainSize is not among {2^14, 2^15, 2^16, 2^17}
    function newEvalDomain(uint256 domainSize) internal pure returns (EvalDomain memory) {
        if (domainSize == 16384) {
            return
                EvalDomain(
                    14,
                    domainSize,
                    0x30638CE1A7661B6337A964756AA75257C6BF4778D89789AB819CE60C19B04001,
                    0x2D965651CDD9E4811F4E51B80DDCA8A8B4A93EE17420AAE6ADAA01C2617C6E85,
                    0x281C036F06E7E9E911680D42558E6E8CF40976B0677771C0F8EEE934641C8410
                );
        } else if (domainSize == 32768) {
            return
                EvalDomain(
                    15,
                    domainSize,
                    0x3063edaa444bddc677fcd515f614555a777997e0a9287d1e62bf6dd004d82001,
                    0x2d1ba66f5941dc91017171fa69ec2bd0022a2a2d4115a009a93458fd4e26ecfb,
                    0x05d33766e4590b3722701b6f2fa43d0dc3f028424d384e68c92a742fb2dbc0b4
                );
        } else if (domainSize == 65536) {
            return
                EvalDomain(
                    16,
                    domainSize,
                    0x30641e0e92bebef818268d663bcad6dbcfd6c0149170f6d7d350b1b1fa6c1001,
                    0x00eeb2cb5981ed45649abebde081dcff16c8601de4347e7dd1628ba2daac43b7,
                    0x0b5d56b77fe704e8e92338c0082f37e091126414c830e4c6922d5ac802d842d4
                );
        } else if (domainSize == 131072) {
            return
                EvalDomain(
                    17,
                    domainSize,
                    0x30643640b9f82f90e83b698e5ea6179c7c05542e859533b48b9953a2f5360801,
                    0x1bf82deba7d74902c3708cc6e70e61f30512eca95655210e276e5858ce8f58e5,
                    0x244cf010c43ca87237d8b00bf9dd50c4c01c7f086bd4e8c920e75251d96f0d22
                );
        } else {
            revert("Poly: size must in 2^{14~17}");
        }
    }

    // This evaluates the vanishing polynomial for this domain at zeta.
    // For multiplicative subgroups, this polynomial is
    // `z(X) = X^self.size - 1`.
    function evaluateVanishingPoly(EvalDomain memory self, uint256 zeta)
        internal
        pure
        returns (uint256 res)
    {
        uint256 p = BN254.R_MOD;
        uint256 logSize = self.logSize;

        assembly {
            switch zeta
            case 0 {
                res := sub(p, 1)
            }
            default {
                res := zeta
                for {
                    let i := 0
                } lt(i, logSize) {
                    i := add(i, 1)
                } {
                    res := mulmod(res, res, p)
                }
                // since zeta != 0 we know that res is not 0
                // so we can safely do a subtraction
                res := sub(res, 1)
            }
        }
    }

    /// @dev Evaluate the lagrange polynomial at point `zeta` given the vanishing polynomial evaluation `vanish_eval`.
    function evaluateLagrangeOne(
        EvalDomain memory self,
        uint256 zeta,
        uint256 vanishEval
    ) internal view returns (uint256 res) {
        if (vanishEval == 0) {
            return 0;
        }

        uint256 p = BN254.R_MOD;
        uint256 divisor;
        uint256 vanishEvalMulSizeInv = self.sizeInv;

        // =========================
        // lagrange_1_eval = vanish_eval / self.size / (zeta - 1)
        // =========================
        assembly {
            vanishEvalMulSizeInv := mulmod(vanishEval, vanishEvalMulSizeInv, p)

            switch zeta
            case 0 {
                divisor := sub(p, 1)
            }
            default {
                divisor := sub(zeta, 1)
            }
        }
        divisor = BN254.invert(divisor);
        assembly {
            res := mulmod(vanishEvalMulSizeInv, divisor, p)
        }
    }

    /// @dev Evaluate public input polynomial at point `zeta`.
    function evaluatePiPoly(
        EvalDomain memory self,
        uint256[] memory pi,
        uint256 zeta,
        uint256 vanishEval
    ) internal view returns (uint256 res) {
        if (vanishEval == 0) {
            return 0;
        }

        uint256 p = BN254.R_MOD;
        uint256 length = pi.length;
        uint256 ithLagrange;
        uint256 divisor;
        uint256 tmp;
        uint256 vanishEvalDivN = self.sizeInv;
        uint256[] memory localDomainElements = domainElements(self, length);

        // vanish_eval_div_n = (zeta^n-1)/n
        assembly {
            vanishEvalDivN := mulmod(vanishEvalDivN, vanishEval, p)
        }

        // Now we need to compute
        //  \sum_{i=0..l} L_{i,H}(zeta) * pub_input[i]
        // where
        // - L_{i,H}(zeta)
        //      = Z_H(zeta) * v_i / (zeta - g^i)
        //      = vanish_eval_div_n * g^i / (zeta - g^i)
        // - v_i = g^i / n
        for (uint256 i = 0; i < length; i++) {
            assembly {
                // tmp points to g^i
                // first 32 bytes of reference is the length of an array
                tmp := mload(add(add(localDomainElements, 0x20), mul(i, 0x20)))
                // vanish_eval_div_n * g^i
                ithLagrange := mulmod(vanishEvalDivN, tmp, p)
                // compute (zeta - g^i)
                divisor := addmod(sub(p, tmp), zeta, p)
            }
            // compute 1/(zeta - g^i)
            divisor = BN254.invert(divisor);
            assembly {
                // tmp points to public input
                tmp := mload(add(add(pi, 0x20), mul(i, 0x20)))
                ithLagrange := mulmod(ithLagrange, tmp, p)
                ithLagrange := mulmod(ithLagrange, divisor, p)

                res := addmod(res, ithLagrange, p)
            }
        }
    }

    /// @dev Generate the domain elements for indexes 0..length
    /// which are essentially g^0, g^1, ..., g^{length-1}
    function domainElements(EvalDomain memory self, uint256 length)
        internal
        pure
        returns (uint256[] memory elements)
    {
        uint256 groupGen = self.groupGen;
        uint256 tmp = 1;
        uint256 p = BN254.R_MOD;
        elements = new uint256[](length);
        assembly {
            if not(iszero(length)) {
                let ptr := add(elements, 0x20)
                let end := add(ptr, mul(0x20, length))
                mstore(ptr, 1)
                ptr := add(ptr, 0x20)
                // for (; ptr < end; ptr += 32) loop through the memory of `elements`
                for {

                } lt(ptr, end) {
                    ptr := add(ptr, 0x20)
                } {
                    tmp := mulmod(tmp, groupGen, p)
                    mstore(ptr, tmp)
                }
            }
        }
    }
}
