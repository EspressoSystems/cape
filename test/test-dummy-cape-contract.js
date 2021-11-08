const { expect } = require("chai");
const { ethers } = require("hardhat");
const common = require("../lib/common");

<<<<<<< HEAD:test/test-dummy-verifier.js
describe("Dummy Verifier", function () {
=======
async function check_gas(
  fun_to_evaluate,
  chunk,
  merkle_trees_update,
  expected_gas_str
) {
  const tx = await fun_to_evaluate(chunk, merkle_trees_update);
  const txReceipt = await tx.wait();

  const gasUsed = txReceipt.gasUsed.toString();
  const expectedGasUsed = ethers.BigNumber.from(expected_gas_str);
  expect(gasUsed).equal(expectedGasUsed);
}

describe("Dummy CAPE contract", function () {
>>>>>>> 0fa1dfd (Records Merkle Tree.):test/test-dummy-cape-contract.js
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

<<<<<<< HEAD:test/test-dummy-verifier.js
    async function check_actual_gas(chunk, merkle_trees_update) {
      const gas = [];
      for (const fun of fun_to_eval) {
        const tx = await fun(chunk, merkle_trees_update);
        const txReceipt = await tx.wait();
        gas.push(txReceipt.gasUsed.toString());
=======
    it("Works with merkle tree update", async function () {
      const expected_gas_array = ["119317", "7058625", "6965094"];

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(fun_to_eval[i], chunk, true, expected_gas_array[i]);
>>>>>>> 0fa1dfd (Records Merkle Tree.):test/test-dummy-cape-contract.js
      }
      return gas;
    }

    it("Works with merkle tree update", async function () {
      const expected_gas = ["119329", "7051382", "6959977"];
      const actual_gas = await check_actual_gas(chunk, true);
      expect(actual_gas).to.deep.equal(expected_gas);
    });

    it("Works with without merkle tree update", async function () {
<<<<<<< HEAD:test/test-dummy-verifier.js
      const expected_gas = ["119317", "759922", "705161"];
      const actual_gas = await check_actual_gas(chunk, false);
      expect(actual_gas).to.deep.equal(expected_gas);
    });

    it("Batch verifier is more efficient than simple verifier when there are enough transactions", async function () {
      const expected_gas = ["167870", "10295854", "10173003"];
=======
      const expected_gas_array = ["119305", "762059", "705172"];

      for (let i = 0; i < fun_to_eval.length; i++) {
        await check_gas(fun_to_eval[i], chunk, false, expected_gas_array[i]);
      }
    });

    it("Batch verifier is more efficient than simple verifier when there are enough transactions", async function () {
      const expected_gas_array = ["167846", "1132912", "1007674"];
>>>>>>> 0fa1dfd (Records Merkle Tree.):test/test-dummy-cape-contract.js

      const N_CAPTX = 3;
      const chunk = common.create_chunk(N_CAPTX);

      const actual_gas = await check_actual_gas(chunk, true);
      expect(actual_gas).to.deep.equal(expected_gas);

      // Batch verification is faster than simple verification
      expect(parseInt(expected_gas[2])).lt(parseInt(expected_gas[1]));
    });
  });
});
