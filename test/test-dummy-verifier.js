const { expect } = require("chai");
const { ethers } = require("hardhat");
const common = require("../lib/common")

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

describe("Dummy Verifier", function () {
  describe("Should compute the gas fee", async function () {
    let owner, fun_to_eval;

    const N_AAPTX = 5;
    const chunk = common.create_chunk(N_AAPTX);

    before(async function () {
      [owner] = await ethers.getSigners();

      const DPV = await ethers.getContractFactory("DummyVerifier");
      const dpv = await DPV.deploy();

      // Polling interval in ms.
      dpv.provider.pollingInterval = 20;

      await dpv.deployed();

      fun_to_eval = [dpv.verify_empty, dpv.verify, dpv.batch_verify];
    });


    it("Works with merkle tree update (Starkware)", async function () {

      const expected_gas_array = ["265025", "11361125", "11100177"];

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(fun_to_eval[i], chunk, true, true,expected_gas_array[i]);
      }

      let best_cost_per_tx = parseInt(expected_gas_array[2])/N_AAPTX;
      expect(best_cost_per_tx).equal(2220035.4);

    });

    it("Works with merkle tree update (NO Starkware)", async function () {

      const expected_gas_array = ["265013", "12541371", "12280400"];

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(fun_to_eval[i], chunk, true, false,expected_gas_array[i]);
      }

      let best_cost_per_tx = parseInt(expected_gas_array[2])/N_AAPTX;
      expect(2456080).equal(best_cost_per_tx);

    });

    it("Works with without merkle tree update)", async function () {

      const expected_gas_array = ["265001", "1876851", "1616829"];

      // Batch verification is faster than simple verification
      expect(parseInt(expected_gas_array[2])).lt(
        parseInt(expected_gas_array[1])
      );

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(fun_to_eval[i], chunk, false, false, expected_gas_array[i]);
      }

      let best_cost_per_tx = parseInt(expected_gas_array[2])/N_AAPTX;
      expect(best_cost_per_tx).equal(323365.8);

    });
  });
});
