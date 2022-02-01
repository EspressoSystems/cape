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

    function evaluateLagrange(
        PolynomialEval.EvalDomain memory self,
        uint256 zeta,
        uint256 vanishEval
    ) public view returns (uint256) {
        return PolynomialEval.evaluateLagrangeOne(self, zeta, vanishEval);
    }

    function evaluatePiPoly(
        PolynomialEval.EvalDomain memory self,
        uint256[] memory pi,
        uint256 zeta,
        uint256 vanishEval
    ) public view returns (uint256) {
        return PolynomialEval.evaluatePiPoly(self, pi, zeta, vanishEval);
    }

    function newEvalDomain(uint256 domainSize)
        public
        pure
        returns (PolynomialEval.EvalDomain memory)
    {
        if (domainSize >= 32768) {
            return PolynomialEval.newEvalDomain(domainSize);
        } else if (domainSize == 32) {
            // support smaller domains for testing
            return
                PolynomialEval.EvalDomain(
                    5,
                    domainSize,
                    0x2EE12BFF4A2813286A8DC388CD754D9A3EF2490635EBA50CB9C2E5E750800001,
                    0x09C532C6306B93D29678200D47C0B2A99C18D51B838EEB1D3EED4C533BB512D0,
                    0x2724713603BFBD790AEAF3E7DF25D8E7EF8F311334905B4D8C99980CF210979D
                );
        } else {
            revert("domain size not supported");
        }
    }
}
