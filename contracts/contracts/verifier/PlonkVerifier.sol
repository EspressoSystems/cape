// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {BN254} from "../libraries/BN254.sol";
import "hardhat/console.sol";
import "../interfaces/IPlonkVerifier.sol";
import {PolynomialEval as Poly} from "../libraries/PolynomialEval.sol";
import "./Transcript.sol";

contract PlonkVerifier is IPlonkVerifier {
    using Transcript for Transcript.TranscriptData;

    // TODO: consider switching this to a smaller coset? currently generated by
    // Jellyfish's `compute_coset_representatives()`
    uint256 private constant _COSET_K0 =
        0x0000000000000000000000000000000000000000000000000000000000000001;
    uint256 private constant _COSET_K1 =
        0x2f8dd1f1a7583c42c4e12a44e110404c73ca6c94813f85835da4fb7bb1301d4a;
    uint256 private constant _COSET_K2 =
        0x1ee678a0470a75a6eaa8fe837060498ba828a3703b311d0f77f010424afeb025;
    uint256 private constant _COSET_K3 =
        0x2042a587a90c187b0a087c03e29c968b950b1db26d5c82d666905a6895790c0a;
    uint256 private constant _COSET_K4 =
        0x2e2b91456103698adf57b799969dea1c8f739da5d8d40dd3eb9222db7c81e881;

    // Parsed from Aztec's Ignition CRS,
    // `beta_h` \in G2 where \beta is the trapdoor, h is G2 generator `BN254.P2()`
    // See parsing code: https://github.com/alxiong/crs
    BN254.G2Point private _betaH =
        BN254.G2Point({
            x0: 0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
            x1: 0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0,
            y0: 0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
            y1: 0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55
        });

    /// The number of wire types of the circuit, TurboPlonk has 5.
    uint256 private constant _NUM_WIRE_TYPES = 5;

    /// @dev polynomial commitment evaluation info.
    struct PcsInfo {
        // a random combiner that was used to combine evaluations at point
        uint256 u; // 0x00
        // the point to be evaluated at
        uint256 evalPoint; // 0x20
        // the shifted point to be evaluated at
        uint256 nextEvalPoint; // 0x40
        // the polynomial evaluation value
        uint256 eval; // 0x60
        // TODO: deliberate on if two arrays is the best choice for `struct ScalarsAndBases` in JF.
        // scalars of poly comm for MSM
        uint256[] commScalars; // 0x80
        // bases of poly comm for MSM
        BN254.G1Point[] commBases; // 0xa0
        // proof of evaluations at point `eval_point`
        BN254.G1Point openingProof; // 0xc0
        // proof of evaluations at point `next_eval_point`
        BN254.G1Point shiftedOpeningProof; // 0xe0
    }

    /// @dev Plonk IOP verifier challenges.
    struct Challenges {
        uint256 alpha; // 0x00
        uint256 beta; // 0x20
        uint256 gamma; // 0x40
        uint256 zeta; // 0x60
        uint256 v; // 0x80
        uint256 u; // 0xa0
    }

    /// @dev Batch verify multiple TurboPlonk proofs.
    function batchVerify(
        VerifyingKey[] memory verifyingKeys,
        uint256[][] memory publicInputs,
        PlonkProof[] memory proofs,
        bytes[] memory extraTranscriptInitMsgs
    ) external view returns (bool) {
        require(
            verifyingKeys.length == proofs.length &&
                publicInputs.length == proofs.length &&
                extraTranscriptInitMsgs.length == proofs.length,
            "Plonk: invalid input param"
        );
        require(proofs.length > 0, "Plonk: need at least 1 proof");

        PcsInfo[] memory pcsInfos = new PcsInfo[](proofs.length);
        for (uint256 i = 0; i < proofs.length; i++) {
            pcsInfos[i] = _preparePcsInfo(
                verifyingKeys[i],
                publicInputs[i],
                proofs[i],
                extraTranscriptInitMsgs[i]
            );
        }

        return _batchVerifyOpeningProofs(pcsInfos);
    }

    function _preparePcsInfo(
        VerifyingKey memory verifyingKey,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        bytes memory extraTranscriptInitMsg
    ) internal view returns (PcsInfo memory res) {
        require(publicInput.length == verifyingKey.numInputs, "Plonk: wrong verifying key");

        Challenges memory chal = _computeChallenges(
            verifyingKey,
            publicInput,
            proof,
            extraTranscriptInitMsg
        );

        Poly.EvalDomain memory domain = Poly.newEvalDomain(verifyingKey.domainSize);
        // compute opening proof in poly comm.
        (
            uint256[] memory commScalars,
            BN254.G1Point[] memory commBases,
            uint256 eval
        ) = _prepareOpeningProof(domain, verifyingKey, publicInput, proof, chal);

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

    // TODO: remove solhint disable
    /* solhint-disable */
    // Compute alpha^2, alpha^3,
    function _computeAlphaPowers(uint256 alpha)
        internal
        pure
        returns (uint256[2] memory alphaPowers)
    {
        // `alpha_bases` is unnecessary since it's just `vec![E::Fr::one()]` here
        uint256 p = BN254.R_MOD;
        assembly {
            let alpha2 := mulmod(alpha, alpha, p)
            mstore(alphaPowers, alpha2)

            let alpha3 := mulmod(alpha, alpha2, p)
            mstore(add(alphaPowers, 0x20), alpha3)
        }
    }

    function _computeChallenges(
        VerifyingKey memory verifyingKey,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        bytes memory extraTranscriptInitMsg
    ) internal pure returns (Challenges memory) {
        // TODO: depends on https://github.com/SpectrumXYZ/cape/issues/171
    }

    /// @dev Compute the constant term of the linearization polynomial
    ///
    /// r_plonk = PI - L1(x) * alpha^2 - alpha * \prod_i=1..m-1 (w_i + beta * sigma_i + gamma) * (w_m + gamma) * z(xw)
    /// where m is the number of wire types.
    function _computeLinPolyConstantTerm(
        Poly.EvalDomain memory domain,
        Challenges memory chal,
        VerifyingKey memory verifyingKey,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        uint256 vanishEval,
        uint256 lagrangeOneEval,
        uint256[2] memory alphaPowers
    ) internal view returns (uint256 res) {
        uint256 piEval = Poly.evaluatePiPoly(domain, publicInput, chal.zeta, vanishEval);
        uint256 perm = _computeLinPolyConstantTermPartialPermEval(chal, proof);
        uint256 p = BN254.R_MOD;

        assembly {
            let alpha := mload(chal)
            let gamma := mload(add(chal, 0x40))
            let alpha2 := mload(alphaPowers)
            let w4 := mload(add(proof, 0x220))
            let permNextEval := mload(add(proof, 0x2c0))

            // \prod_i=1..m-1 (w_i + beta * sigma_i + gamma) * (w_m + gamma) * z(xw)
            perm := mulmod(perm, mulmod(addmod(w4, gamma, p), permNextEval, p), p)
            // PI - L1(x) * alpha^2 - alpha * \prod_i=1..m-1 (w_i + beta * sigma_i + gamma) * (w_m + gamma) * z(xw)
            res := addmod(piEval, sub(p, mulmod(alpha2, lagrangeOneEval, p)), p)
            res := addmod(res, sub(p, mulmod(alpha, perm, p)), p)
        }
    }

    // partial permutation term evaluation, (break out as a function to avoid "Stack too deep" error).
    function _computeLinPolyConstantTermPartialPermEval(
        Challenges memory chal,
        PlonkProof memory proof
    ) internal view returns (uint256 perm) {
        uint256 p = BN254.R_MOD;
        assembly {
            let w0 := mload(add(proof, 0x1a0))
            let w1 := mload(add(proof, 0x1c0))
            let w2 := mload(add(proof, 0x1e0))
            let w3 := mload(add(proof, 0x200))
            let sigma0 := mload(add(proof, 0x240))
            let sigma1 := mload(add(proof, 0x260))
            let sigma2 := mload(add(proof, 0x280))
            let sigma3 := mload(add(proof, 0x2a0))
            let beta := mload(add(chal, 0x20))
            let gamma := mload(add(chal, 0x40))

            // \prod_i=1..m-1 (w_i + beta * sigma_i + gamma)
            perm := 1
            perm := mulmod(perm, addmod(add(w0, gamma), mulmod(beta, sigma0, p), p), p)
            perm := mulmod(perm, addmod(add(w1, gamma), mulmod(beta, sigma1, p), p), p)
            perm := mulmod(perm, addmod(add(w2, gamma), mulmod(beta, sigma2, p), p), p)
            perm := mulmod(perm, addmod(add(w3, gamma), mulmod(beta, sigma3, p), p), p)
        }
    }

    // Compute components in [E]1 and [F]1 used for PolyComm opening verification
    // Returned commitment is a generalization of `[F]1` described in Sec 8.4, step 10 of https://eprint.iacr.org/2019/953.pdf
    // Returned evaluation is the scalar in `[E]1` described in Sec 8.4, step 11 of https://eprint.iacr.org/2019/953.pdf
    //
    // equivalent of JF's https://github.com/SpectrumXYZ/jellyfish/blob/main/plonk/src/proof_system/verifier.rs#L154-L170
    function _prepareOpeningProof(
        Poly.EvalDomain memory domain,
        VerifyingKey memory verifyingKey,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        Challenges memory chal
    )
        internal
        view
        returns (
            uint256[] memory commScalars,
            BN254.G1Point[] memory commBases,
            uint256 eval
        )
    {
        // pre-compute alpha related values
        uint256[2] memory alphaPowers = _computeAlphaPowers(chal.alpha);

        uint256 vanishEval = Poly.evaluateVanishingPoly(domain, chal.zeta);
        (uint256 lagrangeOneEval, uint256 lagrangeNEval) = Poly.evaluateLagrangeOneAndN(
            domain,
            chal.zeta,
            vanishEval
        );

        // compute the constant term of the linearization polynomial
        uint256 linPolyConstant = _computeLinPolyConstantTerm(
            domain,
            chal,
            verifyingKey,
            publicInput,
            proof,
            vanishEval,
            lagrangeOneEval,
            alphaPowers
        );

        // TODO: implement `aggregate_poly_commitments` inline (otherwise would encounter "Stack Too Deep")
        // `aggregate_poly_commitments()` in Jellyfish, but since we are not aggregating multiple,
        // but rather preparing for `[F]1` from a single proof.
        uint256[] memory bufferVAndUvBasis;

        eval = _prepareEvaluations(linPolyConstant, proof, bufferVAndUvBasis);
    }

    // `aggregate_evaluations()` in Jellyfish, but since we are not aggregating multiple, but rather preparing `[E]1` from a single proof.
    // The returned value is the scalar in `[E]1` described in Sec 8.4, step 11 of https://eprint.iacr.org/2019/953.pdf
    function _prepareEvaluations(
        uint256 linPolyConstant,
        PlonkProof memory proof,
        uint256[] memory bufferVAndUvBasis
    ) internal pure returns (uint256 eval) {
        // TODO: https://github.com/SpectrumXYZ/cape/issues/9
    }

    // Batchly verify multiple PCS opening proofs.
    // `open_key` has been assembled from BN254.P1(), BN254.P2() and contract variable _betaH
    function _batchVerifyOpeningProofs(PcsInfo[] memory pcsInfos) internal view returns (bool) {
        uint256 pcsLen = pcsInfos.length;
        uint256 p = BN254.R_MOD;
        // Compute a pseudorandom challenge from the instances
        uint256 r = 1; // for a single proof, no need to use `r` (`r=1` has no effect)
        if (pcsLen > 1) {
            Transcript.TranscriptData memory transcript;
            for (uint256 i = 0; i < pcsLen; i++) {
                transcript.appendChallenge(pcsInfos[i].u);
            }
            r = transcript.getAndAppendChallenge();
        }

        BN254.G1Point memory a1;
        BN254.G1Point memory b1;

        // Compute A := A0 + r * A1 + ... + r^{m-1} * Am
        {
            uint256[] memory scalars = new uint256[](2 * pcsLen);
            BN254.G1Point[] memory bases = new BN254.G1Point[](2 * pcsLen);
            uint256 rBase = 1;
            for (uint256 i = 0; i < pcsLen; i++) {
                scalars[2 * i] = rBase;
                bases[2 * i] = pcsInfos[i].openingProof;

                {
                    uint256 tmp;
                    uint256 u = pcsInfos[i].u;
                    assembly {
                        tmp := mulmod(rBase, u, p)
                    }
                    scalars[2 * i + 1] = tmp;
                }
                bases[2 * i + 1] = pcsInfos[i].shiftedOpeningProof;

                assembly {
                    rBase := mulmod(rBase, r, p)
                }
            }
            a1 = BN254.multiScalarMul(bases, scalars);
        }

        // Compute B := B0 + r * B1 + ... + r^{m-1} * Bm
        {

        }

        b1 = _computePairingB1Term(pcsInfos, r);

        // Check e(A, [x]2) ?= e(B, [1]2)
        return BN254.pairingProd2(a1, _betaH, b1, BN254.P2());
    }

    // TODO: remove the next line
    /* solhint-disable */

    function _computePairingB1Term(PcsInfo[] memory pcsInfos, uint256 combiner)
        internal
        view
        returns (BN254.G1Point memory b1)
    {
        // uint256 pcsLen = pcsInfos.length;
        uint256 p = BN254.R_MOD;

        // Compute B := B0 + r * B1 + ... + r^{m-1} * Bm
        uint256 pcsInfoScalarsBasesLen = pcsInfos[0].commScalars.length;
        uint256 scalarsBasesLenB = (2 + pcsInfoScalarsBasesLen) * pcsInfos.length + 1;
        uint256[] memory scalars = new uint256[](scalarsBasesLenB);
        BN254.G1Point[] memory bases = new BN254.G1Point[](scalarsBasesLenB);
        uint256 sumEvals = 0;
        uint256 idx = 0;

        // assembly {
        //     // return memory position of `ith` slots in a dynamic array
        //     function posInDynamicArray(pointer, ith) -> result {
        //         result := add(pointer, add(0x20, mul(ith, 0x20)))
        //     }

        //     let rBase := 1

        //     for {
        //         let i := 0
        //     } lt(i, pcsLen) {
        //         i := add(i, 1)
        //     } {
        //         for {
        //             let j := 0
        //         } lt(j, pcsInfoScalarsBasesLen) {
        //             j := add(j, 1)
        //         } {
        //             // scalars[idx] = (rBase * pcsInfos[i].commScalars[j]) % BN254.R_MOD;
        //             let s := mload(posInDynamicArray(mload(posInDynamicArray(pcsInfos, i)), j))
        //             mstore(posInDynamicArray(scalars, idx), mulmod(rBase, s, p))
        //             // bases[idx] = pcsInfos[i].commBases[j];
        //             mstore(
        //                 posInDynamicArray(bases, idx),
        //                 mload(posInDynamicArray(mload(posInDynamicArray(pcsInfos, i)), j))
        //             )
        //             // idx += 1;
        //             idx := add(idx, 1)
        //         }

        //         // scalars[idx] = (rBase * pcsInfos[i].evalPoint) % BN254.R_MOD;
        //         let evalPoint := mload(add(posInDynamicArray(pcsInfos, i), 0x20))
        //         mstore(posInDynamicArray(scalars, idx), mulmod(rBase, evalPoint, p))
        //         // bases[idx] = pcsInfos[i].openingProof;
        //         mstore(
        //             posInDynamicArray(bases, idx),
        //             mload(add(posInDynamicArray(pcsInfos, i), 0xc0))
        //         )
        //         // idx += 1;
        //         idx := add(idx, 1)

        //         // scalars[idx] = (rBase * pcsInfos[i].u * pcsInfos[i].nextEvalPoint) % BN254.R_MOD;
        //         let u := mload(posInDynamicArray(pcsInfos, i))
        //         let nextEvalPoint := mload(add(posInDynamicArray(pcsInfos, i), 0x40))
        //         mstore(
        //             posInDynamicArray(scalars, idx),
        //             mulmod(rBase, mulmod(u, nextEvalPoint, p), p)
        //         )
        //         // bases[idx] = pcsInfos[i].shiftedOpeningProof;
        //         mstore(
        //             posInDynamicArray(bases, idx),
        //             mload(add(posInDynamicArray(pcsInfos, i), 0xe0))
        //         )
        //         // idx += 1;
        //         idx := add(idx, 1)

        //         // sumEvals = (sumEvals + rBase * pcsInfos[i].eval) % BN254.R_MOD;
        //         let eval := mload(add(posInDynamicArray(pcsInfos, i), 0x60))
        //         sumEvals := addmod(sumEvals, mulmod(rBase, eval, p), p)
        //         // rBase = (rBase * r) % BN254.R_MOD;
        //         rBase := mulmod(rBase, combiner, p)
        //     }
        // }

        uint256 rBase = 1;
        for (uint256 i = 0; i < pcsInfos.length; i++) {
            for (uint256 j = 0; j < pcsInfoScalarsBasesLen; j++) {
                {
                    // scalars[idx] = (rBase * pcsInfos[i].commScalars[j]) % BN254.R_MOD;
                    uint256 s = pcsInfos[i].commScalars[j];
                    uint256 tmp;
                    assembly {
                        tmp := mulmod(rBase, s, p)
                    }
                    scalars[idx] = tmp;
                }
                bases[idx] = pcsInfos[i].commBases[j];
                idx += 1;
            }

            {
                // scalars[idx] = (rBase * pcsInfos[i].evalPoint) % BN254.R_MOD;
                uint256 evalPoint = pcsInfos[i].evalPoint;
                uint256 tmp;
                assembly {
                    tmp := mulmod(rBase, evalPoint, p)
                }
                scalars[idx] = tmp;
            }
            bases[idx] = pcsInfos[i].openingProof;
            idx += 1;

            {
                // scalars[idx] = (rBase * pcsInfos[i].u * pcsInfos[i].nextEvalPoint) % BN254.R_MOD;
                uint256 u = pcsInfos[i].u;
                uint256 nextEvalPoint = pcsInfos[i].nextEvalPoint;
                uint256 tmp;
                assembly {
                    tmp := mulmod(rBase, mulmod(u, nextEvalPoint, p), p)
                }
                scalars[idx] = tmp;
            }
            bases[idx] = pcsInfos[i].shiftedOpeningProof;
            idx += 1;

            {
                // sumEvals = (sumEvals + rBase * pcsInfos[i].eval) % BN254.R_MOD;
                // rBase = (rBase * combiner) % BN254.R_MOD;
                uint256 eval = pcsInfos[i].eval;
                assembly {
                    sumEvals := addmod(sumEvals, mulmod(rBase, eval, p), p)
                    rBase := mulmod(rBase, combiner, p)
                }
            }
        }
        scalars[idx] = BN254.negate(sumEvals);
        bases[idx] = BN254.P1();
        b1 = BN254.negate(BN254.multiScalarMul(bases, scalars));
    }
}
