// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

const { expect } = require("chai");
const { ethers } = require("hardhat");

/*
import { RecordsMerkleTree } from "../typechain-types";
*/

describe("Records Merkle Tree tests", function () {
  let recordsMerkleTree: any;
  let rmtFactory: {
    deploy: (arg0: number) => any | PromiseLike<any>;
  };

  beforeEach(async function () {
    let rescue = await (await ethers.getContractFactory("RescueLib")).deploy();
    rmtFactory = await ethers.getContractFactory("RecordsMerkleTree", {
      libraries: {
        RescueLib: rescue.address,
      },
    });
  });

  it("inserts all 27 leaves into a merkle tree of height 3", async function () {
    let TREE_HEIGHT = 3;
    recordsMerkleTree = await rmtFactory.deploy(TREE_HEIGHT);

    let elems = [
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
      26,
    ];

    // Insert all these elements does not trigger an error
    let tx = await recordsMerkleTree.updateRecordsMerkleTree(elems);
    await tx.wait();
  });

  it("shows that inserting too many leaves triggers an error", async function () {
    let TREE_HEIGHT = 3;
    recordsMerkleTree = await rmtFactory.deploy(TREE_HEIGHT);

    let elems = [
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
      26, 27, 28,
    ];

    await expect(recordsMerkleTree.updateRecordsMerkleTree(elems)).to.be.revertedWith(
      "The tree is full."
    );
  });
});
