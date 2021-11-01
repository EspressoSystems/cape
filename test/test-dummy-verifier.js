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

describe("Dummy Verifier", function () {
  describe("Should compute the gas fee", async function () {
    let owner, fun_to_eval;

    const N_AAPTX = 2;
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
      const expected_gas_array = ["119481", "4338307", "4244569"];

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

    it("Works with merkle tree update (NO Starkware)", async function () {
      const expected_gas_array = ["119469", "6539630", "6482991"];

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

    it("Works with without merkle tree update)", async function () {
      const expected_gas_array = ["119457", "762249", "705279"];

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

    it("Batch verifier is more efficient than simple verifier when there are enough transactions", async function () {
      const expected_gas_array = ["167998", "1133102", "1007781"];

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
  });
});
