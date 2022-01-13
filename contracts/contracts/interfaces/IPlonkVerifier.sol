// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../libraries/BN254.sol";

interface IPlonkVerifier {
    // Flatten out TurboPlonk proof
    struct PlonkProof {
        BN254.G1Point wire0; // input wire poly com
        BN254.G1Point wire1;
        BN254.G1Point wire2;
        BN254.G1Point wire3;
        BN254.G1Point wire4; // output wire poly com
        BN254.G1Point prodPerm; // product permutation poly com
        BN254.G1Point split0; // split quotient poly com
        BN254.G1Point split1;
        BN254.G1Point split2;
        BN254.G1Point split3;
        BN254.G1Point split4;
        BN254.G1Point zeta; // witness poly com for aggregated opening at `zeta`
        BN254.G1Point zetaOmega; // witness poly com for shifted opening at `zeta * \omega`
        uint256 wireEval0; // wire poly eval at `zeta`
        uint256 wireEval1;
        uint256 wireEval2;
        uint256 wireEval3;
        uint256 wireEval4;
        uint256 sigmaEval0; // extended permutation (sigma) poly eval at `zeta`
        uint256 sigmaEval1;
        uint256 sigmaEval2;
        uint256 sigmaEval3; // last (sigmaEval4) is saved by Maller Optimization
        uint256 prodPermZetaOmegaEval; // product permutation poly eval at `zeta * \omega`
    }

    // The verifying key for Plonk proofs.
    struct VerifyingKey {
        uint256 domainSize;
        uint256 numInputs;
        BN254.G1Point sigma0; // commitment to extended perm (sigma) poly
        BN254.G1Point sigma1;
        BN254.G1Point sigma2;
        BN254.G1Point sigma3;
        BN254.G1Point sigma4;
        BN254.G1Point q1; // commitment to selector poly
        BN254.G1Point q2; // first 4 are linear combination selector
        BN254.G1Point q3;
        BN254.G1Point q4;
        BN254.G1Point qM12; // multiplication selector for 1st, 2nd wire
        BN254.G1Point qM34; // multiplication selector for 3rd, 4th wire
        BN254.G1Point qO; // output selector
        BN254.G1Point qC; // constant term selector
        BN254.G1Point qH1; // rescue selector qH1 * w_ai^5
        BN254.G1Point qH2; // rescue selector qH2 * w_bi^5
        BN254.G1Point qH3; // rescue selector qH3 * w_ci^5
        BN254.G1Point qH4; // rescue selector qH4 * w_di^5
        BN254.G1Point qEcc; // elliptic curve selector
        uint256 k0; // coset representative
        uint256 k1;
        uint256 k2;
        uint256 k3;
        uint256 k4;
    }

    /// @dev Verify a single TurboPlonk proof.
    function verify(
        VerifyingKey memory verifyingKey,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        bytes memory extraTranscriptInitMsg
    ) external returns (bool);

    // TODO: To be refined, we might be able to merge some part of verifying keys.
    // /// @dev Batch verify multiple TurboPlonk proofs.
    // function batchVerify(
    //     bytes[] memory verifyingKey,
    //     uint256[][] memory publicInputs,
    //     PlonkProof[] memory proofs,
    //     bytes[] memory extraTranscriptInitMsgs
    // ) external returns (bool);
}
