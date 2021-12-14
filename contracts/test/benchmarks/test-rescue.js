const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Rescue benchmarks", function () {
  describe("Gas spent for computing the Rescue function", function () {
    for (const [contractName, gas] of [
      ["TestRescue", 87164],
      ["TestRescueNonOptimized", 620060],
    ]) {
      it(`checks gas usage of ${contractName}.hash`, async function () {
        const Rescue = await ethers.getContractFactory(contractName);
        let rescueContract = await Rescue.deploy();

        // Polling interval in ms.
        rescueContract.provider.pollingInterval = 20;

        await rescueContract.deployed();

        const doNothingTx = await rescueContract.doNothing();
        const doNothingtxReceipt = await doNothingTx.wait();
        let doNothingGasUsed = doNothingtxReceipt.gasUsed;

        const rescueTx = await rescueContract.hash(10, 15, 20);
        const rescueTxReceipt = await rescueTx.wait();
        let rescueGasUsed = rescueTxReceipt.gasUsed;

        let rescueOnly = rescueGasUsed - doNothingGasUsed;

        expect(rescueOnly).to.be.equal(gas);
      });
    }
  });
});
