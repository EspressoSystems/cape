// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {BN254} from "../libraries/BN254.sol";
import {PlonkVerifier as V} from "../verifier/PlonkVerifier.sol";
import {PolynomialEval as Poly} from "../libraries/PolynomialEval.sol";

contract TestPlonkVerifier is V {
    function computeLinPolyConstantTerm(
        Poly.EvalDomain memory domain,
        Challenges memory chal,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        Poly.EvalData memory evalData
    ) public view returns (uint256 res) {
        return V._computeLinPolyConstantTerm(domain, chal, publicInput, proof, evalData);
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

    function computeChallenges(
        V.VerifyingKey memory verifyingKey,
        uint256[] memory publicInput,
        V.PlonkProof memory proof,
        bytes memory extraTranscriptInitMsg
    ) public pure returns (V.Challenges memory) {
        return V._computeChallenges(verifyingKey, publicInput, proof, extraTranscriptInitMsg);
    }

    function linearizationScalarsAndBases(
        V.VerifyingKey memory verifyingKey,
        V.Challenges memory challenge,
        Poly.EvalData memory evalData,
        V.PlonkProof memory proof
    ) public pure returns (BN254.G1Point[] memory bases, uint256[] memory scalars) {
        //returns (BN254.G1Point[15] memory bases, uint256[15] memory scalars) {
        return V._linearizationScalarsAndBases(verifyingKey, challenge, evalData, proof);
    }

    function preparePolyCommitments(
        VerifyingKey memory verifyingKey,
        Challenges memory chal,
        Poly.EvalData memory evalData,
        PlonkProof memory proof
    )
        public
        pure
        returns (
            uint256[] memory commScalars,
            BN254.G1Point[] memory commBases,
            uint256[10] memory bufferVAndUvBasis
        )
    {
        return V._preparePolyCommitments(verifyingKey, chal, evalData, proof);
    }

    // helper so that test code doesn't have to deploy both PlonkVerifier.sol and BN254.sol
    function multiScalarMul(BN254.G1Point[] memory bases, uint256[] memory scalars)
        public
        view
        returns (BN254.G1Point memory)
    {
        return BN254.multiScalarMul(bases, scalars);
    }
}
