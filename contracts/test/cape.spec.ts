// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

import { expect } from "chai";
import { ethers } from "hardhat";

const TREE_HEIGHT = 24;
const N_ROOTS = 1000;

describe("CAPE", function () {
  describe("Handling of nullifiers", async function () {
    let cape: any;

    beforeEach(async function () {
      let rescue = await (await ethers.getContractFactory("RescueLib")).deploy();
      let verifyingKeys = await (await ethers.getContractFactory("VerifyingKeys")).deploy();
      let plonkVerifier = await (await ethers.getContractFactory("PlonkVerifier")).deploy();

      let merkleTree = await (
        await ethers.getContractFactory("RecordsMerkleTree", {
          libraries: {
            RescueLib: rescue.address,
          },
        })
      ).deploy(TREE_HEIGHT);

      let capeFactory = await ethers.getContractFactory("TestCAPE", {
        libraries: {
          RescueLib: rescue.address,
          VerifyingKeys: verifyingKeys.address,
        },
      });

      cape = await capeFactory.deploy(N_ROOTS, plonkVerifier.address, merkleTree.address);

      let tx = await merkleTree.transferOwnership(cape.address);
      await tx.wait();
    });

    it("is possible to check for non-membership", async function () {
      let elem = ethers.utils.randomBytes(32);
      expect(await cape.nullifiers(elem)).to.be.false;

      let tx = await cape.publish(elem);
      await tx.wait();
      expect(await cape.nullifiers(elem)).to.be.true;
    });

    it("is possible to publish several nullifiers", async function () {
      let elem1 = ethers.utils.randomBytes(32);
      let elem2 = ethers.utils.randomBytes(32);
      expect(elem1).not.equal(elem2);

      let tx = await cape.publish(elem1);
      await tx.wait();
      expect(await cape.publish(elem2)).not.to.throw;
    });
  });
});
