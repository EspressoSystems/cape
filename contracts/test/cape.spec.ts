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
      expect(await cape.callStatic._hasNullifierAlreadyBeenPublished(elem)).to.be.true;

      await cape._insertNullifier(elem);
      expect(await cape.callStatic._hasNullifierAlreadyBeenPublished(elem)).to.be.false;
    });

    it("is possible to insert several elements", async function () {
      let elem1 = ethers.utils.randomBytes(32);
      let elem2 = ethers.utils.randomBytes(32);
      expect(elem1).not.equal(elem2);

      await cape._insertNullifier(elem1);

      expect(await cape._insertNullifier(elem1)).not.to.throw;

      expect(await cape._insertNullifier(elem2)).not.to.throw;
    });

    it("updates the commitment to the set of nullifiers correctly.", async function () {
      let init_commitment = await cape.callStatic.getNullifierSetCommitment();
      let expected_init_commitment =
        "0x0000000000000000000000000000000000000000000000000000000000000000";
      expect(init_commitment.toString()).equal(expected_init_commitment);

      let encoder = new ethers.utils.AbiCoder();

      let null1 = ethers.utils.randomBytes(32);

      await cape._insertNullifier(null1);
      let new_commitment = await cape.callStatic.getNullifierSetCommitment();

      let expected_new_commitment = ethers.utils.keccak256(
        encoder.encode(["bytes32", "bytes32"], [init_commitment, null1])
      );
      expect(new_commitment.toString()).equal(expected_new_commitment);

      let null2 = ethers.utils.randomBytes(32);

      await cape._insertNullifier(null2);

      expected_new_commitment = ethers.utils.keccak256(
        encoder.encode(["bytes32", "bytes32"], [new_commitment, null2])
      );

      new_commitment = await cape.callStatic.getNullifierSetCommitment();

      expect(new_commitment.toString()).equal(expected_new_commitment);
    });
  });
});
