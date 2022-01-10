import { expect } from "chai";
import { ethers } from "hardhat";
import { TestCAPE } from "../typechain-types";

describe("CAPE", function () {
  describe("Handling of nullifiers", async function () {
    let cape: TestCAPE;

    beforeEach(async function () {
      let rescue = await (await ethers.getContractFactory("RescueLib")).deploy();
      let capeFactory = await ethers.getContractFactory("TestCAPE", {
        libraries: {
          RescueLib: rescue.address,
        },
      });
      const TREE_HEIGHT = 20;
      const N_ROOTS = 3;
      cape = await capeFactory.deploy(TREE_HEIGHT, N_ROOTS);
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
