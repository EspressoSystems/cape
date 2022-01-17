// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {BN254} from "../libraries/BN254.sol";
import {PlonkVerifier as V} from "../verifier/PlonkVerifier.sol";
import {PolynomialEval as Poly} from "../libraries/PolynomialEval.sol";

contract TestPlonkVerifier is V {
    function computeAlphaPowers(uint256 alpha)
        public
        pure
        returns (uint256[2] memory alphaPowers)
    {
        return V._computeAlphaPowers(alpha);
    }

    function computeLinPolyConstantTerm(
        Poly.EvalDomain memory domain,
        Challenges memory chal,
        VerifyingKey memory verifyingKey,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        uint256 vanishEval,
        uint256 lagrangeOneEval,
        uint256[2] memory alphaPowers
    ) public view returns (uint256 res) {
        return
            V._computeLinPolyConstantTerm(
                domain,
                chal,
                verifyingKey,
                publicInput,
                proof,
                vanishEval,
                lagrangeOneEval,
                alphaPowers
            );
    }

    function prepareEvaluations(
        uint256 linPolyConstant,
        PlonkProof memory proof,
        uint256[10] memory bufferVAndUvBasis
    ) public pure returns (uint256 eval) {
        return V._prepareEvaluations(linPolyConstant, proof, bufferVAndUvBasis);
    }

    function batchVerifyOpeningProofs(PcsInfo[] memory pcsInfos) public view returns (bool) {
        return V._batchVerifyOpeningProofs(pcsInfos);
    }
}
