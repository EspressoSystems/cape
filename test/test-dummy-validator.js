const { expect } = require("chai");
const { ethers } = require("hardhat");
const common = require("../lib/common");

async function check_gas(
  fun_to_evaluate,
  chunk,
  merkle_trees_update,
  is_starkware,
  expected_gas_str
) {
  const tx = await fun_to_evaluate(chunk, merkle_trees_update, is_starkware);
  const txReceipt = await tx.wait();

  const gasUsed = txReceipt.gasUsed.toString();
  const expectedGasUsed = ethers.BigNumber.from(expected_gas_str);
  expect(gasUsed).equal(expectedGasUsed);
}

describe("Dummy Validator" + "", function () {
  describe("Should compute the gas fee", async function () {
    let owner, fun_to_eval;

    const N_AAPTX = 2;
    const chunk = common.create_chunk(N_AAPTX);

    before(async function () {
      [owner] = await ethers.getSigners();

      const DPV = await ethers.getContractFactory("DummyValidator");
      const dpv = await DPV.deploy();

      // Polling interval in ms.
      dpv.provider.pollingInterval = 20;

      await dpv.deployed();

      fun_to_eval = [dpv.verify_empty, dpv.verify, dpv.batch_verify];
    });

    it.skip("Works with merkle tree update (Starkware)", async function () {
      const expected_gas_array = ["119726", "8436088", "8380305"];

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(
          fun_to_eval[i],
          chunk,
          true,
          true,
          expected_gas_array[i]
        );
      }
    });

    it.skip("Works with merkle tree update (NO Starkware)", async function () {
      const expected_gas_array = ["121714", "10843220", "10787535"];

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(
          fun_to_eval[i],
          chunk,
          true,
          false,
          expected_gas_array[i]
        );
      }
    });

    it.skip("Works with without merkle tree update)", async function () {
      const expected_gas_array = ["121702", "762259", "705278"];

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(
          fun_to_eval[i],
          chunk,
          false,
          false,
          expected_gas_array[i]
        );
      }
    });

    it.skip("Batch verifier is more efficient than simple verifier when there are enough transactions", async function () {
      const expected_gas_array = ["170243", "1133123", "1007780"];

      const N_AAPTX = 3;
      const chunk = common.create_chunk(N_AAPTX);

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(
          fun_to_eval[i],
          chunk,
          false,
          false,
          expected_gas_array[i]
        );
      }

      // Batch verification is faster than simple verification
      expect(parseInt(expected_gas_array[2])).lt(
        parseInt(expected_gas_array[1])
      );
    });

    it("measures how much it costs to send the nullifiers proofs to the smart contract in order to update the Merkle root", async function () {
      const DPV = await ethers.getContractFactory("DummyValidator");
      const dpv = await DPV.deploy();

      // Polling interval in ms.
      dpv.provider.pollingInterval = 20;

      await dpv.deployed();

      let N_NULLIFIERS = 4; // Equal to the number of inputs
      let NULLIFIERS_MERKLE_TREE_HEIGHT = 256;

      let hash_value = ethers.utils.randomBytes(32);

      let nullifiers_proofs = [];
      for (let i = 0; i < N_NULLIFIERS * NULLIFIERS_MERKLE_TREE_HEIGHT; i++) {
        nullifiers_proofs.push(hash_value);
      }

      let nullifiers = [];
      for (let i = 0; i < N_NULLIFIERS; i++) {
        nullifiers.push(hash_value);
      }

      let new_root = hash_value;

      let tx = await dpv.multi_insert(nullifiers_proofs, nullifiers, new_root);
      const txReceipt = await tx.wait();

      const gasUsed = txReceipt.gasUsed;

      expect(gasUsed).gt(500000);
    });

    it("measures how much it costs to store directly the list of nullifiers inside the contract", async function () {
      const DPV = await ethers.getContractFactory("DummyValidator");
      const dpv = await DPV.deploy();

      // Polling interval in ms.
      dpv.provider.pollingInterval = 20;

      await dpv.deployed();

      let N_NULLIFIERS = 4; // Equal to the number of inputs

      let nullifiers = [];
      for (let i = 0; i < N_NULLIFIERS; i++) {
        let value = ethers.utils.randomBytes(32);
        nullifiers.push(value);
      }

      let tx = await dpv.nullifier_check_insert(nullifiers);
      let txReceipt = await tx.wait();
      const gasUsed = txReceipt.gasUsed;
      expect(gasUsed).lt(120000);

      // The transaction is reverted because the nullifiers are already inserted
      await expect(dpv.nullifier_check_insert(nullifiers)).to.be.revertedWith(
        ""
      );
    });
  });
});
