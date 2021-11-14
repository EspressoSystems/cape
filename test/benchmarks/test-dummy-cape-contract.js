const { expect } = require("chai");
const { ethers } = require("hardhat");
const common = require("../../lib/common");

describe("Dummy CAPE", function () {
  describe("Should compute the gas fee", async function () {
    let owner, fun_to_eval;

    const N_CAPTX = 2;
    const chunk = common.create_chunk(N_CAPTX);

    before(async function () {
      [owner] = await ethers.getSigners();

      const DPV = await ethers.getContractFactory("DummyCAPE");
      const dpv = await DPV.deploy();

      // Polling interval in ms.
      dpv.provider.pollingInterval = 20;

      await dpv.deployed();

      fun_to_eval = [dpv.verifyEmpty, dpv.verify, dpv.batchVerify];
    });

    async function check_actual_gas(chunk, merkle_trees_update) {
      const gas = [];
      for (const fun of fun_to_eval) {
        const tx = await fun(chunk, merkle_trees_update);
        const txReceipt = await tx.wait();
        gas.push(txReceipt.gasUsed.toString());
      }
      return gas;
    }

    it("Works with merkle tree update", async function () {
      const expected_gas = ["119274", "2134795", "2043013"];
      const actual_gas = await check_actual_gas(chunk, true);
      expect(actual_gas).to.deep.equal(expected_gas);
    });

    it("Works with without merkle tree update", async function () {
      const expected_gas = ["119262", "759681", "704894"];
      const actual_gas = await check_actual_gas(chunk, false);
      expect(actual_gas).to.deep.equal(expected_gas);
    });

    it("Batch verifier is more efficient than simple verifier when there are enough transactions", async function () {
      const expected_gas = ["167815", "3091030", "2967916"];

      const N_CAPTX = 3;
      const chunk = common.create_chunk(N_CAPTX);

      const actual_gas = await check_actual_gas(chunk, true);
      expect(actual_gas).to.deep.equal(expected_gas);

      // Batch verification is faster than simple verification
      expect(parseInt(expected_gas[2])).lt(parseInt(expected_gas[1]));
    });
  });
});
