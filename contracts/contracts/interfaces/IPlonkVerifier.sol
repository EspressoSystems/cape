// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../libraries/BN254.sol";

interface IPlonkVerifier {
    // Flatten out TurboPlonk proof
    struct PlonkProof {
        // the first 5 are 4 inputs and 1 output wire poly commmitments
        // i.e., batch_proof.wires_poly_comms_vec.iter()
        // wire0 is 32 bytes which is a pointer to BN254.G1Point
        BN254.G1Point wire0; // 0x00
        BN254.G1Point wire1; // 0x20
        BN254.G1Point wire2; // 0x40
        BN254.G1Point wire3; // 0x60
        BN254.G1Point wire4; // 0x80
        // the next one is the  product permutation poly commitment
        // i.e., batch_proof.prod_perm_poly_comms_vec.iter()
        BN254.G1Point prodPerm; // 0xA0
        // the next 5 are split quotient poly commmitments
        // i.e., batch_proof.split_quot_poly_comms
        BN254.G1Point split0; // 0xC0
        BN254.G1Point split1; // 0xE0
        BN254.G1Point split2; // 0x100
        BN254.G1Point split3; // 0x120
        BN254.G1Point split4; // 0x140
        // witness poly com for aggregated opening at `zeta`
        // i.e., batch_proof.opening_proof
        BN254.G1Point zeta; // 0x160
        // witness poly com for shifted opening at `zeta * \omega`
        // i.e., batch_proof.shifted_opening_proof
        BN254.G1Point zetaOmega; // 0x180
        // wire poly eval at `zeta`
        uint256 wireEval0; // 0x1A0
        uint256 wireEval1; // 0x1C0
        uint256 wireEval2; // 0x1E0
        uint256 wireEval3; // 0x200
        uint256 wireEval4; // 0x220
        // extended permutation (sigma) poly eval at `zeta`
        // last (sigmaEval4) is saved by Maller Optimization
        uint256 sigmaEval0; // 0x240
        uint256 sigmaEval1; // 0x260
        uint256 sigmaEval2; // 0x280
        uint256 sigmaEval3; // 0x2A0
        // product permutation poly eval at `zeta * \omega`
        uint256 prodPermZetaOmegaEval; // 0x2C0
    }

    // The verifying key for Plonk proofs.
    struct VerifyingKey {
        uint256 domainSize; // 0x00
        uint256 numInputs; // 0x20
        // commitment to extended perm (sigma) poly
        BN254.G1Point sigma0; // 0x40
        BN254.G1Point sigma1; // 0x60
        BN254.G1Point sigma2; // 0x80
        BN254.G1Point sigma3; // 0xA0
        BN254.G1Point sigma4; // 0xC0
        // commitment to selector poly
        // first 4 are linear combination selector
        BN254.G1Point q1; // 0xE0
        BN254.G1Point q2; // 0x100
        BN254.G1Point q3; // 0x120
        BN254.G1Point q4; // 0x140
        // multiplication selector for 1st, 2nd wire
        BN254.G1Point qM12; // 0x160
        // multiplication selector for 3rd, 4th wire
        BN254.G1Point qM34; // 0x180
        // output selector
        BN254.G1Point qO; // 0x1A0
        // constant term selector
        BN254.G1Point qC; // 0x1C0
        // rescue selector qH1 * w_ai^5
        BN254.G1Point qH1; // 0x1E0
        // rescue selector qH2 * w_bi^5
        BN254.G1Point qH2; // 0x200
        // rescue selector qH3 * w_ci^5
        BN254.G1Point qH3; // 0x220
        // rescue selector qH4 * w_di^5
        BN254.G1Point qH4; // 0x240
        // elliptic curve selector
        BN254.G1Point qEcc; // 0x260
    }

    /// @dev Batch verify multiple TurboPlonk proofs.
    /// @param verifyingKeys An array of verifying keys
    /// @param publicInputs A two-dimensional array of public inputs.
    /// @param proofs An array of Plonk proofs
    /// @param extraTranscriptInitMsgs An array of bytes from
    /// transcript initialization messages
    /// @return _ A boolean that is true for successful verification, false otherwise
    function batchVerify(
        VerifyingKey[] memory verifyingKeys,
        uint256[][] memory publicInputs,
        PlonkProof[] memory proofs,
        bytes[] memory extraTranscriptInitMsgs
    ) external view returns (bool);
}
