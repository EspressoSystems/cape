const { expect } = require("chai");
const { ethers } = require("hardhat");
const common = require("../lib/common");

describe("Nullifiers Merkle tree", function () {
  it("should compute correctly the hash functions", async function () {
    let nf_merkle_tree = await common.deployNullifierMerkleTreeContract();
    let _res = await nf_merkle_tree.callStatic.elem_hash(10000);
  });

  it.skip("should compute the terminal node value", async function () {
    const contract = await common.deployNullifierMerkleTreeContract();

    // fails at
    //    height=262 against geth
    //    heigth=? against arbitrum dev node
    // but it's not entirely deterministic

    for (let height = 262; height < 263; height++) {
      //console.error("height", height);
      let tx = await contract.terminalNodeValueNonEmpty({
        isEmptySubtree: false,
        height: height,
        elem: ethers.utils.randomBytes(32),
      });
      await tx.wait();
    }
  });

  describe("Conversions", function () {
    let contract;

    beforeEach(async function () {
      const Contract = await ethers.getContractFactory("BLAKE2b");
      contract = await Contract.deploy();
      await contract.deployed();
    });

    it.skip("converts u64 -> b32", async function () {
      const z = [1, 2, 3, 4, 5, 6, 7, 8];
      const ret = await contract.Uint64ArrayToBytes32Array(z);
      expect(ret).to.deep.equal([
        "0x0000000000000001000000000000000200000000000000030000000000000004",
        "0x0000000000000005000000000000000600000000000000070000000000000008",
      ]);
    });

    it.skip("handles 64 bit little endian words", async function () {
      // little endian
      // const numbers = [1,2,3,4];
      const uintHex =
        "0x0000000000000001000000000000000200000000000000030000000000000004";
      const bytes = [
        "010000000000000000",
        "020000000000000000",
        "030000000000000000",
        "040000000000000000",
      ];
      // store this in an uint256
      const asUint = ethers.BigNumber.from(uintHex);
      const ret = await contract.u64toByte(asUint);
      console.error(ret);
    });

    it.skip("converts u256 -> b32", async function () {
      const z = [1, 2, 3, 4];
      const ret = await contract.Uint256ArrayToBytesArray(z);
      expect(ret).to.deep.equal([
        "0x0000000000000000000000000000000000000000000000000000000000000001",
        "0x0000000000000000000000000000000000000000000000000000000000000002",
        "0x0000000000000000000000000000000000000000000000000000000000000003",
        "0x0000000000000000000000000000000000000000000000000000000000000004",
      ]);
    });

    it.skip("converts u128 -> b8", async function () {
      const z = ethers.BigNumber.from("0x10000000000000023000000000000004");
      const ret = await contract.Uint128ToBytes8(z);
      expect(ret).to.deep.equal(["0x1000000000000002", "0x3000000000000004"]);
    });

    it("converts b32 -> u64", async function () {
      const z = [
        "0x0000000000000000000000000000000000000000000000000000000000000001",
        "0x0000000000000000000000000000000000000000000000000000000000000002",
      ];
      const ret = await contract.Bytes32ArrayToUint64Array(z);
      const expected = [0, 0, 0, 1, 0, 0, 0, 2];
      expected.forEach((e, i) => expect(ret[i]).to.equal(e));
    });

    it("F function works with test vector", async function () {
      // https://eips.ethereum.org/EIPS/eip-152
      // test vector 5
      const z =
        "0000000c48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001";
      //   "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923";
      const expected = [
        "0xba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1",
        "0x7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923",
      ];
      const slices = [
        [[0, 4]],
        [
          [4, 36],
          [36, 68],
        ],
        [
          [68, 100],
          [100, 132],
          [132, 164],
          [164, 196],
        ],
        [
          [196, 204],
          [204, 212],
        ],
        [[212, 213]],
      ];

      args = slices.map((items) => {
        if (items.length == 1) {
          const [start, end] = items[0];
          return "0x" + z.slice(2 * start, 2 * end);
        } else {
          return items.map(
            ([start, end]) => "0x" + z.slice(2 * start, 2 * end)
          );
        }
      });

      const ret = await contract.F(...args);
      expect(ret).to.deep.equal(expected);
    });

    it.skip("Computes the full hash", async function () {
      // Just useful for logging in solidity
      const m =
        "0x000000000000000000000000000000000000000000000000000000000000000001";
      const k =
        "0x000000000000000000000000000000000000000000000000000000000000000000";
      const ret = await contract.callStatic.blake2b(m, k, 64);
      console.error(ret);
    });

    it("Test endian", async function () {
      const ret = await contract.callStatic.endian(1);
      console.error(ret);
    });

    it("encodePacked", async function () {
      const ret = await contract.callStatic.encodePacked(12);
      console.error(ret);
    });
  });
});
