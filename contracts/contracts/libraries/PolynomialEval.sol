//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {BN254} from "../libraries/BN254.sol";

library PolynomialEval {
    /// @dev a Radix 2 Evaluation Domain
    struct EvalDomain {
        uint256 logSize;
        uint256 size; // Size of the domain as a field element
        uint256 sizeInv; // Inverse of the size in the field
        uint256 groupGen; // A generator of the subgroup
        uint256 groupGenInv; // Inverse of the generator of the subgroup
    }

    /// @dev Create a new Radix2EvalDomain with `domainSize` which should be power of 2.
    /// @dev Will revert if domainSize is not among {2^15, 2^16, 2^17}
    function newEvalDomain(uint256 domainSize) internal pure returns (EvalDomain memory) {
        if (domainSize == 32768) {
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
            revert("Poly: size must in 2^{15,16,17}");
        }
    }

    // This evaluates the vanishing polynomial for this domain at zeta.
    // For multiplicative subgroups, this polynomial is
    // `z(X) = X^self.size - 1`.
    function evaluateVanishingPoly(EvalDomain memory self, uint256 zeta)
        internal
        pure
        returns (uint256)
    {
        uint256 p = BN254.R_MOD;
        if (zeta == 0) {
            return (p - 1);
        }

        uint256 res;
        res = zeta;
        assembly {
            // repreating 15 times
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
            res := mulmod(res, res, p)
        }
        if (self.logSize == 15) {} else if (self.logSize == 16) {
            assembly {
                res := mulmod(res, res, p)
            }
        } else if (self.logSize == 17) {
            assembly {
                res := mulmod(res, res, p)
                res := mulmod(res, res, p)
            }
        } else {
            revert("Poly: size not in 2^{15, 16, 17}");
        }

        // since zeta != 0 we know that res is not 0
        // so we can safely do a subtraction
        res--;

        return (res);
    }

    /// @dev Evaluate the first and the last lagrange polynomial at point `zeta` given the vanishing polynomial evaluation `vanish_eval`.
    function evaluateLagrangeOneAndN(
        EvalDomain memory self,
        uint256 zeta,
        uint256 vanishEval
    ) internal view returns (uint256, uint256) {
        if (vanishEval == 0) {
            return (0, 0);
        }

        uint256 p = BN254.R_MOD;
        uint256 divisor;
        uint256 res1;
        uint256 res2;
        uint256 groupGenInv = self.groupGenInv;
        uint256 vanishEvalMulSizeInv = self.sizeInv;

        assembly {
            vanishEvalMulSizeInv := mulmod(vanishEval, vanishEvalMulSizeInv, p)
        }

        // =========================
        // lagrange_1_eval = vanish_eval / self.size / (zeta - 1)
        // =========================
        if (zeta == 0) {
            divisor = p - 1;
        } else {
            divisor = zeta - 1;
        }
        // QUESTION: is there an assembly instruction for this?
        divisor = BN254.invert(divisor);
        assembly {
            res1 := mulmod(vanishEvalMulSizeInv, divisor, p)
        }

        // =========================
        // lagrange_n_eval = vanish_eval * self.group_gen_inv / self.size / (zeta - self.group_gen_inv)
        // =========================
        if (zeta < groupGenInv) {
            divisor = zeta + p - groupGenInv;
        } else {
            divisor = zeta - groupGenInv;
        }
        // QUESTION: is there an assembly instruction for this?
        divisor = BN254.invert(divisor);
        assembly {
            res2 := mulmod(vanishEvalMulSizeInv, groupGenInv, p)
            res2 := mulmod(res2, divisor, p)
        }

        return (res1, res2);
    }

    /// @dev Evaluate public input polynomial at point `zeta`.
    function evaluatePiPoly(
        EvalDomain memory self,
        uint256[] memory pi,
        uint256 zeta,
        uint256 vanishEval
    ) internal view returns (uint256) {
        if (vanishEval == 0) {
            return (0);
        }

        uint256 p = BN254.R_MOD;
        uint256 length = pi.length;
        uint256 res = 0;
        uint256 ithLagrange;
        uint256 divisor;
        uint256 tmp;
        uint256 vanishEvalDivN = self.size;
        uint256[] memory localDomainElements = domainElements(self, length);

        // vanish_eval_div_n = (zeta^n-1)/n
        vanishEvalDivN = BN254.invert(vanishEvalDivN);
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
            // tmp points to g^i
            tmp = localDomainElements[i];
            assembly {
                // vanish_eval_div_n * g^i
                ithLagrange := mulmod(vanishEvalDivN, tmp, p)
            }

            // compute 1/(zeta - g^i)
            if (zeta >= tmp) {
                divisor = zeta - tmp;
            } else {
                divisor = zeta + p - tmp;
            }
            divisor = BN254.invert(divisor);

            // tmp points to public input
            tmp = pi[i];
            assembly {
                ithLagrange := mulmod(ithLagrange, tmp, p)
                ithLagrange := mulmod(ithLagrange, divisor, p)
            }
            res += ithLagrange;
        }
        return (res);
    }

    /// @dev Generate the domain elements for indexes 0..length
    /// which are essentially g^0, g^1, ..., g^{length-1}
    function domainElements(EvalDomain memory self, uint256 length)
        internal
        pure
        returns (uint256[] memory)
    {
        uint256 groupGen = self.groupGen;
        uint256 tmp = 1;
        uint256 p = BN254.R_MOD;
        uint256[] memory elements = new uint256[](length);

        if (length != 0) {
            elements[0] = 1;
            for (uint256 i = 1; i < length; i++) {
                assembly {
                    tmp := mulmod(tmp, groupGen, p)
                }
                elements[i] = tmp;
            }
        }
        return (elements);
    }
}
