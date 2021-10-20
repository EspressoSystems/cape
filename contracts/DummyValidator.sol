//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import {Curve} from "./BN254.sol";
import {Rescue} from "./Rescue.sol";

contract DummyValidator {
    uint256 RECORDS_TREE_HEIGHT = 25;
    uint256 NULLIFIERS_TREE_HEIGHT = 256; // TODO Change to 512
    uint256 AAPTX_SIZE = 3000; // Must be the same as in the javascript testing code
    uint256 N_INPUTS = 4; // Number of AAP inputs per transactions corresponding to a transaction of roughly 3 KB
    uint256 N_OUTPUTS = 5; // Number of AAP outputs per transactions of roughly 3 KB
    uint256 dummy_var = 1; // Just to avoid compilers warnings

    function verify_empty(
        bytes memory chunk,
        bool merkle_trees_update,
        bool is_starkware
    ) public returns (bool) {
        assert(chunk[0] >= 0); // Just to avoid compilers warnings
        assert(merkle_trees_update == merkle_trees_update); // Just to avoid compilers warnings
        assert(is_starkware == is_starkware);
        dummy_var = 0; // Just to avoid compilers warnings

        return true;
    }

    function verify(
        bytes memory chunk,
        bool merkle_trees_update,
        bool is_starkware
    ) public returns (bool) {
        // Count the number of transactions
        uint256 n_aaptx = chunk.length / AAPTX_SIZE;

        // n_aaptx pairing check
        for (uint256 i = 0; i < n_aaptx; i++) {
            run_pairing_check();
        }

        // Cost of prepare_pcs_info
        prepare_pcs_info(n_aaptx);

        if (merkle_trees_update) {
            update_merkle_trees(n_aaptx, is_starkware);
        }

        return true;
    }

    function prepare_pcs_info(uint256 n_aaptx) private view {
        // $n_aaptx$ multi-exp in G1 of size $c$ where c=32
        // (Empirically 29=<c<=36. See rust code call `prepare_pcs_info` in PlonkKzgSnark.batch_verify)

        uint256 c = 32;
        for (uint256 i = 0; i < n_aaptx; i++) {
            run_multi_exp_g1(c);
        }
    }

    function update_nullifiers_tree() private view returns (bytes32) {
        for (uint256 i = 0; i < NULLIFIERS_TREE_HEIGHT * N_INPUTS; i++) {
            // Compute blake2 hash
            string memory left = "a";
            string memory right = "b";

            return keccak256(abi.encodePacked(left, right, new bytes(64)));
        }

        return 0;
    }

    /*
        Simulation of the batch insertion of elements in the ternary Merkle tree
        based on Joe's observation:
        Instead of inserting elements one by one which would cost H*N where N is the number of
        elements and H is the height of the tree, the idea is to insert the elements in batch,
        saving by doing so many calls to the rescue hash function.
        The cost is N + N/3 + N/9 + ... + N/(3^H) which tends to 1.5 N when N grows.
        So we approximate the cost of inserting N elements  in the tree by the cost of computing 1.5 N rescue hash functions.
     */
    function update_records_tree_batch(uint256 n_aaptx, bool is_starkware)
        private
        view
    {
        uint256 TOTAL_COST_BATCH_INSERTION = (3 * n_aaptx * N_OUTPUTS) / 2;

        for (uint256 i = 0; i < TOTAL_COST_BATCH_INSERTION; i++) {
            // Computes rescue hash
            uint256 a = 7878754242;
            uint256 b = 468777777777776575;
            uint256 c = 87875474574;
            Rescue.hash(a, b, c, is_starkware);
        }
    }

    function update_merkle_trees(uint256 n_aaptx, bool is_starkware) public {
        // For the nullifier tree we insert the leaves one by one
        for (uint256 i = 0; i < n_aaptx; i++) {
            update_nullifiers_tree();
        }

        // For the record tree we insert the records in batch
        update_records_tree_batch(n_aaptx, is_starkware);
        dummy_var = 0; // Just to avoid compilers warnings
    }

    function batch_verify(
        bytes memory chunk,
        bool merkle_trees_update,
        bool is_starkware
    ) public returns (bool) {
        // Count the number of transactions
        uint256 aaptx_size = 3000;
        uint256 n_aaptx = chunk.length / aaptx_size;

        // We lower bound the complexity by
        // 1 pairing check
        // 2  multi exp in G1 of size $n_aaptx$ (See rust code PlonkKzgSnark.batch_verify)
        // Cost of prepare_pcs_info(n_aaptx)

        // 2 multi exp in G1 of size $n_aaptx$
        run_multi_exp_g1(n_aaptx);
        run_multi_exp_g1(n_aaptx);

        // 1 pairing check
        run_pairing_check();

        prepare_pcs_info(n_aaptx);

        if (merkle_trees_update) {
            update_merkle_trees(n_aaptx, is_starkware);
        }

        return true;
    }

    function run_pairing_check() private view returns (bool) {
        Curve.G1Point memory g1 = Curve.P1();
        Curve.G2Point memory g2 = Curve.P2();

        Curve.G1Point[] memory points1 = new Curve.G1Point[](1);
        points1[0] = g1;

        Curve.G2Point[] memory points2 = new Curve.G2Point[](1);
        points2[0] = g2;
        bool res = Curve.pairing(points1, points2);
        return res;
    }

    // TODO use proper multiexp opcode
    function run_multi_exp_g1(uint256 size) private view {
        for (uint256 i = 0; i < size; i++) {
            // Group scalar multiplications
            Curve.G1Point memory g1 = Curve.P1();
            uint256 scalar1 = 545454; // TODO use bigger scalar
            Curve.G1Point memory p1 = Curve.g1mul(g1, scalar1);

            assert(p1.X == p1.X); // Just to avoid compiler warning

            // (size-1) group additions
            if (i >= 1) {
                Curve.G1Point memory p2 = Curve.g1add(g1, g1);
                assert(p2.X == p2.X); // Just to avoid compiler warning
            }
        }
    }
}
