// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

import { expect } from "chai";
import { ethers } from "hardhat";
import { BigNumber, BigNumberish } from "ethers";

describe("AccumulatingArray", function () {
  let contract: any;

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
