import { TestBN254 } from "../typechain-types";
import { expect } from "chai";
import { ethers } from "hardhat";
import { G1PointStruct } from "../typechain-types/TestBN254";

describe("bn254", function () {
  let contract: TestBN254;

  beforeEach(async () => {
    const factory = await ethers.getContractFactory("TestBN254");
    contract = await factory.deploy();
  });

  it("should add two G1 points", async function () {
    const zero: G1PointStruct = { X: "0", Y: "0" };
    let res = await contract.callStatic.g1Add(zero, zero);
    // Returns a merged array/object [0, 0, X: 0, Y: 0]
    expect(res.X).to.equal("0");
    expect(res.Y).to.equal("0");
  });

  describe("from little endian bytes", function () {
    const bytes =
      "0x04e95224a7b8351ac459f6d844d853c5ef0265ea5d6a99725fec141a6f247a9e7a3692414235f303994992fc0c92ac39";
    const expected =
      "15788341253387758421406605173323632835302684606984257561298532746213789356263";

    it("can convert from little endian bytes", async function () {
      const result = await contract.callStatic.fromLeBytesModOrder(bytes);
      expect(result).to.equal(expected);
    });
  });
});
