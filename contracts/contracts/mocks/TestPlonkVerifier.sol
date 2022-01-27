// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {BN254} from "../libraries/BN254.sol";
import {PlonkVerifier as V} from "../verifier/PlonkVerifier.sol";
import {PolynomialEval as Poly} from "../libraries/PolynomialEval.sol";
import {TestPolynomialEval as TestPoly} from "../mocks/TestPolynomialEval.sol";
import "hardhat/console.sol";

contract TestPlonkVerifier is V, TestPoly {
    function computeLinPolyConstantTerm(
        Challenges memory chal,
        PlonkProof memory proof,
        Poly.EvalData memory evalData
    ) public pure returns (uint256 res) {
        return V._computeLinPolyConstantTerm(chal, proof, evalData);
    }

    function prepareEvaluations(
        uint256 linPolyConstant,
        PlonkProof memory proof,
        uint256[] memory scalars
    ) public pure returns (uint256 eval) {
        return V._prepareEvaluations(linPolyConstant, proof, scalars);
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
        bases = new BN254.G1Point[](30);
        scalars = new uint256[](30);

        V._linearizationScalarsAndBases(verifyingKey, challenge, evalData, proof, bases, scalars);
    }

    function preparePolyCommitments(
        VerifyingKey memory verifyingKey,
        Challenges memory chal,
        Poly.EvalData memory evalData,
        PlonkProof memory proof
    ) public pure returns (uint256[] memory commScalars, BN254.G1Point[] memory commBases) {
        commBases = new BN254.G1Point[](30);
        commScalars = new uint256[](30);
        V._preparePolyCommitments(verifyingKey, chal, evalData, proof, commScalars, commBases);
    }

    // helper so that test code doesn't have to deploy both PlonkVerifier.sol and BN254.sol
    function multiScalarMul(BN254.G1Point[] memory bases, uint256[] memory scalars)
        public
        view
        returns (BN254.G1Point memory)
    {
        return BN254.multiScalarMul(bases, scalars);
    }

    function preparePcsInfo(
        VerifyingKey memory verifyingKey,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        bytes memory extraTranscriptInitMsg
    ) public view returns (PcsInfo memory res) {
        require(publicInput.length == verifyingKey.numInputs, "Plonk: wrong verifying key");

        Challenges memory chal = V._computeChallenges(
            verifyingKey,
            publicInput,
            proof,
            extraTranscriptInitMsg
        );

        // NOTE: the only difference with actual code
        Poly.EvalDomain memory domain = newEvalDomain(verifyingKey.domainSize);
        // pre-compute evaluation data
        Poly.EvalData memory evalData = Poly.evalDataGen(domain, chal.zeta, publicInput);

        // compute opening proof in poly comm.
        uint256[] memory commScalars = new uint256[](30);
        BN254.G1Point[] memory commBases = new BN254.G1Point[](30);

        uint256 eval = _prepareOpeningProof(
            verifyingKey,
            evalData,
            proof,
            chal,
            commScalars,
            commBases
        );

        uint256 zeta = chal.zeta;
        uint256 omega = domain.groupGen;
        uint256 p = BN254.R_MOD;
        uint256 zetaOmega;
        assembly {
            zetaOmega := mulmod(zeta, omega, p)
        }

        res = PcsInfo(
            chal.u,
            zeta,
            zetaOmega,
            eval,
            commScalars,
            commBases,
            proof.zeta,
            proof.zetaOmega
        );
    }

    function testBatchVerify(
        VerifyingKey[] memory verifyingKeys,
        uint256[][] memory publicInputs,
        PlonkProof[] memory proofs,
        bytes[] memory extraTranscriptInitMsgs
    ) public view returns (bool) {
        require(
            verifyingKeys.length == proofs.length &&
                publicInputs.length == proofs.length &&
                extraTranscriptInitMsgs.length == proofs.length,
            "Plonk: invalid input param"
        );
        require(proofs.length > 0, "Plonk: need at least 1 proof");

        PcsInfo[] memory pcsInfos = new PcsInfo[](proofs.length);
        for (uint256 i = 0; i < proofs.length; i++) {
            // validate proofs are proper group/field elements
            V._validateProof(proofs[i]);

            // validate public input are all proper scalar fields
            for (uint256 j = 0; j < publicInputs[i].length; j++) {
                BN254.validateScalarField(publicInputs[i][j]);
            }

            // NOTE: only difference with actual code
            pcsInfos[i] = preparePcsInfo(
                verifyingKeys[i],
                publicInputs[i],
                proofs[i],
                extraTranscriptInitMsgs[i]
            );
        }

        return _batchVerifyOpeningProofs(pcsInfos);
    }
}
