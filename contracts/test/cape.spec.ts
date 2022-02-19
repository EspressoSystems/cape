import { expect } from "chai";
import { ethers } from "hardhat";

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
      const N_ROOTS = 1000;
      cape = await capeFactory.deploy(TREE_HEIGHT, N_ROOTS, plonkVerifier.address);
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
