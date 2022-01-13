// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {PolynomialEval} from "../libraries/PolynomialEval.sol";

contract TestPolynomialEval {
    function evaluateVanishingPoly(PolynomialEval.EvalDomain memory self, uint256 zeta)
        public
        pure
        returns (uint256)
    {
        return PolynomialEval.evaluateVanishingPoly(self, zeta);
    }

    function evaluateLagrangeOneAndN(
        PolynomialEval.EvalDomain memory self,
        uint256 zeta,
        uint256 vanishEval
    ) public view returns (uint256, uint256) {
        return PolynomialEval.evaluateLagrangeOneAndN(self, zeta, vanishEval);
    }

    function evaluatePiPoly(
        PolynomialEval.EvalDomain memory self,
        uint256[] memory pi,
        uint256 zeta,
        uint256 vanishEval
    ) public view returns (uint256) {
        return PolynomialEval.evaluatePiPoly(self, pi, zeta, vanishEval);
    }
}
