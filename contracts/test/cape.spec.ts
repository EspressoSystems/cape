import { expect } from "chai";
import { ethers } from "hardhat";

/*
TODO: fix import issue

Generating typings for: 27 artifacts in dir: typechain-types for target: ethers-v5
Successfully generated 47 typings!
Compilation finished successfully
An unexpected error occurred:

contracts/test/cape.spec.ts(3,26): error TS2307: Cannot find module '../typechain-types' or its corresponding type declarations.

import { TestCAPE } from "../typechain-types";
*/

describe("CAPE", function () {
  describe("Handling of nullifiers", async function () {
    let cape: any;

    beforeEach(async function () {
      let rescue = await (await ethers.getContractFactory("RescueLib")).deploy();
      let verifyingKeys = await (await ethers.getContractFactory("VerifyingKeys")).deploy();
      let plonkVerifier = await (await ethers.getContractFactory("PlonkVerifier")).deploy();
      let capeFactory = await ethers.getContractFactory("TestCAPE", {
        libraries: {
          RescueLib: rescue.address,
          VerifyingKeys: verifyingKeys.address,
        },
      });

      const TREE_HEIGHT = 24;
      const N_ROOTS = 10;
      cape = await capeFactory.deploy(TREE_HEIGHT, N_ROOTS, plonkVerifier.address);
    });

    it("is possible to check for non-membership", async function () {
      let elem = ethers.utils.randomBytes(32);
      expect(await cape.nullifiers(elem)).to.be.false;

      let tx = await cape.insertNullifier(elem);
      await tx.wait();
      expect(await cape.nullifiers(elem)).to.be.true;
    });

    it("is possible to insert several elements", async function () {
      let elem1 = ethers.utils.randomBytes(32);
      let elem2 = ethers.utils.randomBytes(32);
      expect(elem1).not.equal(elem2);

      let tx = await cape.insertNullifier(elem1);
      await tx.wait();
      expect(await cape.insertNullifier(elem2)).not.to.throw;
    });
  });
});
