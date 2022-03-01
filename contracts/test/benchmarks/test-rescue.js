const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Rescue benchmarks", function () {
  describe("Gas spent for computing the Rescue function", function () {
    for (const contractName of ["TestRescue", "TestRescueNonOptimized"]) {
      it(`checks gas usage of ${contractName}.hash`, async function () {
        const libraries = {};
        if (contractName == "TestRescue") {
          let rescueLib = await (await ethers.getContractFactory("RescueLib")).deploy();
          libraries["RescueLib"] = rescueLib.address;
        }
        const factory = await ethers.getContractFactory(contractName, { libraries });

        let rescueContract = await factory.deploy();

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

        const commitTx = await rescueContract.commit([
          BigInt(10),
          BigInt(15),
          BigInt(20),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
        ]);
        console.log(commitTx);
        const commitTxReceipt = await commitTx.wait();
        let commitGasUsed = commitTxReceipt.gasUsed;
        console.log("Rescue gas of ", contractName, ": ", rescueOnly);

        expect(rescueOnly).to.be.equal(gas);
      });

      it(`checks gas usage of ${contractName}.hash on a potentially overflowing input`, async function () {
        const libraries = {};
        if (contractName == "TestRescue") {
          let rescueLib = await (await ethers.getContractFactory("RescueLib")).deploy();
          libraries["RescueLib"] = rescueLib.address;
        }
        const factory = await ethers.getContractFactory(contractName, { libraries });

        let rescueContract = await factory.deploy();

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

        const commitTx = await rescueContract.commit([
          BigInt(10),
          BigInt(15),
          BigInt(20),
          (BigInt(1) << BigInt(256)) - BigInt(1),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
          BigInt(0),
        ]);
        console.log(commitTx);
        const commitTxReceipt = await commitTx.wait();
        let commitGasUsed = commitTxReceipt.gasUsed;
        console.log("Rescue gas of ", contractName, ": ", rescueOnly);

        expect(rescueOnly).to.be.equal(gas);
      });
    }
  });
});
