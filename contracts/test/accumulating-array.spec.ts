import { expect } from "chai";
import { ethers } from "hardhat";
import { BigNumber, BigNumberish } from "ethers";

import { TestAccumulatingArray } from "../typechain-types";

describe("AccumulatingArray", function () {
  let contract: TestAccumulatingArray;

  beforeEach(async () => {
    const factory = await ethers.getContractFactory("TestAccumulatingArray");
    contract = await factory.deploy();
  });

  it("Accumulates correctly", async function () {
    async function check(
      arrays: BigNumberish[][],
      maxLength: BigNumberish,
      expected: BigNumberish[]
    ) {
      let result = await contract.accumulate(arrays, maxLength);
      expect(result).to.deep.equal(expected.map(BigNumber.from));

      result = await contract.accumulateWithIndividuals(arrays, maxLength);
      expect(result).to.deep.equal(expected.map(BigNumber.from));
    }

    await check([], 0, []);
    await check([[2]], 1, [2]);
    await check([[1], [2]], 2, [1, 2]);
    await check([[1, 2], [3]], 3, [1, 2, 3]);
    await check(
      [
        [1, 2],
        [3, 4],
      ],
      4,
      [1, 2, 3, 4]
    );
  });

  it("Reverts if the max length is exceeded", async function () {
    await expect(contract.accumulate([[1]], 0)).to.be.reverted;
    await expect(contract.accumulateWithIndividuals([[1]], 0)).to.be.reverted;

    await expect(contract.accumulate([[1, 2], [3]], 2)).to.be.reverted;
    await expect(contract.accumulateWithIndividuals([[1, 2], [3]], 2)).to.be.reverted;
  });
});
