// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {BN254} from "../libraries/BN254.sol";
import {PlonkVerifier} from "../verifier/PlonkVerifier.sol";
import {PolynomialEval} from "../libraries/PolynomialEval.sol";

contract TestPlonkVerifier {
    function testEvaluateVanishingPoly(PolynomialEval.EvalDomain memory self, uint256 zeta)
        public
        pure
        returns (uint256)
    {
        return PolynomialEval.evaluateVanishingPoly(self, zeta);
    }

    function testEvaluateLagrangeOneAndN(
        PolynomialEval.EvalDomain memory self,
        uint256 zeta,
        uint256 vanishEval
    ) public view returns (uint256, uint256) {
        return PolynomialEval.evaluateLagrangeOneAndN(self, zeta, vanishEval);
    }
}
