const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Nullifiers Merkle tree", function () {
  let contract;

  beforeEach(async function () {
    const Contract = await ethers.getContractFactory("NullifiersMerkleTree");
    contract = await Contract.deploy();
    await contract.deployed();
    contract.provider.pollingInterval = 20;
  });

  it("should compute correctly the hash functions", async function () {
    let _res = await contract.callStatic.elem_hash(10000);
  });

  describe("bitvec", function () {
    it("works for zeros", async function () {
      const bytes = Array(32).fill(0);
      expect(await contract.to_bool_array(bytes)).to.deep.equal(
        Array(256).fill(false)
      );
    });

    it("works for ones", async function () {
      const bytes = Array(32).fill(255);
      expect(await contract.to_bool_array(bytes)).to.deep.equal(
        Array(256).fill(true)
      );
    });

    it("works for simple [0, ..., 0, 1]", async function () {
      const bytes = Array(32).fill(0);
      bytes[31] = 1;
      expected = Array(256).fill(false);
      expected[255] = true;
      expect(await contract.to_bool_array(bytes)).to.deep.equal(expected);
    });

    it("works for simple [1, 0, ..., 0]", async function () {
      const bytes = Array(32).fill(0);
      bytes[0] = 128;
      expected = Array(256).fill(false);
      expected[0] = true;
      expect(await contract.to_bool_array(bytes)).to.deep.equal(expected);
    });

    it("works for simple [0, 1, ..., 0]", async function () {
      const bytes = Array(32).fill(0);
      bytes[0] = 64;
      expected = Array(256).fill(false);
      expected[1] = true;
      expect(await contract.to_bool_array(bytes)).to.deep.equal(expected);
    });

    it("works with second byte", async function () {
      const bytes = Array(32).fill(0);
      bytes[1] = 128;
      expected = Array(256).fill(false);
      expected[8] = true;
      expect(await contract.to_bool_array(bytes)).to.deep.equal(expected);
    });

    it("works for generic", async function () {
      const bytes = ethers.utils.randomBytes(32);
      const bytes_as_bits = [];
      for (const byte of bytes) {
        const bits = byte.toString(2).padStart(8, "0");
        const arr = Array.from(bits).map((s) => Boolean(Number(s)));
        bytes_as_bits.push(arr);
      }
      const bits = [].concat(...bytes_as_bits);
      expect(await contract.to_bool_array(bytes)).to.deep.equal(bits);
    });
  });

  // TODO unskip this test
  it.skip("should compute the terminal node value", async function () {
    const [owner] = await ethers.getSigners();

    const contract = deploy();
    // fails at
    //    height=147 against geth
    //    heigth=147 against arbitrum dev node
    // but it's not entirely deterministic
    for (let height = 146; height < 512; height += 1) {
      console.error("height", height);
      let tx = await contract.terminalNodeValueNonEmpty({
        isEmptySubtree: false,
        height: height,
        elem: ethers.utils.randomBytes(32),
      });
      await tx.wait();
    }
  });
});
