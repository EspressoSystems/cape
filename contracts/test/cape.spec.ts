import { expect } from "chai";
import { ethers } from "hardhat";
import { TestCAPE } from "../typechain-types";

describe("CAPE", function () {
  describe("Handling of nullifiers", async function () {
    let cape: TestCAPE;

    beforeEach(async function () {
      let capeFactory = await ethers.getContractFactory("TestCAPE");
      cape = await capeFactory.deploy();
    });

    it("is possible to check for non-membership", async function () {
      let elem = ethers.utils.randomBytes(32);
      expect(await cape.nullifiers(elem)).to.be.false;

      await cape._insertNullifier(elem);
      expect(await cape.nullifiers(elem)).to.be.true;
    });

    it("is possible to insert several elements", async function () {
      let elem1 = ethers.utils.randomBytes(32);
      let elem2 = ethers.utils.randomBytes(32);
      expect(elem1).not.equal(elem2);

      await cape._insertNullifier(elem1);
      expect(await cape._insertNullifier(elem2)).not.to.throw;
    });
  });
});
