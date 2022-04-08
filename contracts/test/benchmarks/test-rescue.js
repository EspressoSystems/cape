// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

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
      });

      it(`checks gas usage of ${contractName}.commit on a potentially overflowing input`, async function () {
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
        console.log("About to hash");

        const rescueTx = await rescueContract.hash(10, 15, 20);
        const rescueTxReceipt = await rescueTx.wait();
        let rescueGasUsed = rescueTxReceipt.gasUsed;

        let rescueOnly = rescueGasUsed - doNothingGasUsed;

        console.log("About to commit");

        expect(
          rescueContract.commit([
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
          ])
        ).to.be.reverted;
      });
    }
  });
});
