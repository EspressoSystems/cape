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
      let rescueContract;

      beforeEach(async function () {
        const libraries = {};
        if (contractName == "TestRescue") {
          let rescueLib = await (await ethers.getContractFactory("RescueLib")).deploy();
          libraries["RescueLib"] = rescueLib.address;
        }
        const factory = await ethers.getContractFactory(contractName, { libraries });

        rescueContract = await factory.deploy();

        // Polling interval in ms.
        rescueContract.provider.pollingInterval = 20;

        await rescueContract.deployed();
      });

      it(`checks gas usage of ${contractName}.hash`, async function () {
        const doNothingTx = await rescueContract.doNothing();
        const doNothingtxReceipt = await doNothingTx.wait();
        const doNothingGasUsed = doNothingtxReceipt.gasUsed;

        const rescueGasUsed = await rescueContract.estimateGas.hash(10, 15, 20);

        const rescueOnly = rescueGasUsed - doNothingGasUsed;
        console.log("Rescue gas of ", contractName, ": ", rescueOnly);
      });

      it(`check ${contractName}.commit works for non-overflowing input`, async function () {
        expect(
          rescueContract.commit([
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
          ])
        ).to.not.be.reverted;
      });

      it(`check ${contractName}.commit reverts for potentially overflowing input`, async function () {
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
