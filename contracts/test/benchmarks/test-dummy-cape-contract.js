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

      fun_to_eval = [dpv.verifyEmpty, dpv.verify];
    });

    async function check_actual_gas(chunk) {
      const gas = [];
      for (const fun of fun_to_eval) {
        const tx = await fun(chunk);
        const txReceipt = await tx.wait();
        gas.push(txReceipt.gasUsed.toString());
      }
      return gas;
    }

    it("shows an estimation of the cost of validating a block", async function () {
      const expected_gas = ["119004", "2079873"];
      const actual_gas = await check_actual_gas(chunk);
      expect(actual_gas).to.deep.equal(expected_gas);
    });
  });
});
