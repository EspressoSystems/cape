pragma solidity ^0.8.0;

import "hardhat/console.sol";
import "solidity-bytes-utils/contracts/BytesLib.sol";
import "./BLAKE2b_Constants.sol";

contract BLAKE2b is BLAKE2_Constants {
    struct BLAKE2b_ctx {
        uint256[4] b; //input buffer
        uint64[8] h; //chained state
        uint128 t; //total bytes
        uint64 c; //Size of b
        uint256 outlen; //digest output size
    }

    // Mixing Function
    function G(
        uint64[16] memory v,
        uint256 a,
        uint256 b,
        uint256 c,
        uint256 d,
        uint64 x,
        uint64 y
    ) private {
        // Dereference to decrease memory reads
        uint64 va = v[a];
        uint64 vb = v[b];
        uint64 vc = v[c];
        uint64 vd = v[d];

        //Optimised mixing function
        assembly {
            // v[a] := (v[a] + v[b] + x) mod 2**64
            va := addmod(add(va, vb), x, 0x10000000000000000)
            //v[d] := (v[d] ^ v[a]) >>> 32
            vd := xor(
                div(xor(vd, va), 0x100000000),
                mulmod(xor(vd, va), 0x100000000, 0x10000000000000000)
            )
            //v[c] := (v[c] + v[d])     mod 2**64
            vc := addmod(vc, vd, 0x10000000000000000)
            //v[b] := (v[b] ^ v[c]) >>> 24
            vb := xor(
                div(xor(vb, vc), 0x1000000),
                mulmod(xor(vb, vc), 0x10000000000, 0x10000000000000000)
            )
            // v[a] := (v[a] + v[b] + y) mod 2**64
            va := addmod(add(va, vb), y, 0x10000000000000000)
            //v[d] := (v[d] ^ v[a]) >>> 16
            vd := xor(
                div(xor(vd, va), 0x10000),
                mulmod(xor(vd, va), 0x1000000000000, 0x10000000000000000)
            )
            //v[c] := (v[c] + v[d])     mod 2**64
            vc := addmod(vc, vd, 0x10000000000000000)
            // v[b] := (v[b] ^ v[c]) >>> 63
            vb := xor(
                div(xor(vb, vc), 0x8000000000000000),
                mulmod(xor(vb, vc), 0x2, 0x10000000000000000)
            )
        }

        v[a] = va;
        v[b] = vb;
        v[c] = vc;
        v[d] = vd;
    }

    function compress(BLAKE2b_ctx memory ctx, bool last) internal {
        //TODO: Look into storing these as uint256[4]
        uint64[16] memory v;
        uint64[16] memory m;

        for (uint256 i = 0; i < 8; i++) {
            v[i] = ctx.h[i]; // v[:8] = h[:8]
            v[i + 8] = IV[i]; // v[8:] = IV
        }

        //
        v[12] = v[12] ^ uint64(ctx.t % 2**64); //Lower word of t
        v[13] = v[13] ^ uint64(ctx.t / 2**64);

        if (last) v[14] = ~v[14]; //Finalization flag

        uint64 mi; //Temporary stack variable to decrease memory ops
        uint256 b; // Input buffer

        for (uint256 i = 0; i < 16; i++) {
            //Operate 16 words at a time
            uint256 k = i % 4; //Current buffer word
            mi = 0;
            if (k == 0) {
                b = ctx.b[i / 4]; //Load relevant input into buffer
            }

            //Extract relevent input from buffer
            assembly {
                mi := and(
                    div(b, exp(2, mul(64, sub(3, k)))),
                    0xFFFFFFFFFFFFFFFF
                )
            }

            //Flip endianness
            m[i] = getWords(mi);
        }

        //Mix m

        G(v, 0, 4, 8, 12, m[0], m[1]);
        G(v, 1, 5, 9, 13, m[2], m[3]);
        G(v, 2, 6, 10, 14, m[4], m[5]);
        G(v, 3, 7, 11, 15, m[6], m[7]);
        G(v, 0, 5, 10, 15, m[8], m[9]);
        G(v, 1, 6, 11, 12, m[10], m[11]);
        G(v, 2, 7, 8, 13, m[12], m[13]);
        G(v, 3, 4, 9, 14, m[14], m[15]);

        G(v, 0, 4, 8, 12, m[14], m[10]);
        G(v, 1, 5, 9, 13, m[4], m[8]);
        G(v, 2, 6, 10, 14, m[9], m[15]);
        G(v, 3, 7, 11, 15, m[13], m[6]);
        G(v, 0, 5, 10, 15, m[1], m[12]);
        G(v, 1, 6, 11, 12, m[0], m[2]);
        G(v, 2, 7, 8, 13, m[11], m[7]);
        G(v, 3, 4, 9, 14, m[5], m[3]);

        G(v, 0, 4, 8, 12, m[11], m[8]);
        G(v, 1, 5, 9, 13, m[12], m[0]);
        G(v, 2, 6, 10, 14, m[5], m[2]);
        G(v, 3, 7, 11, 15, m[15], m[13]);
        G(v, 0, 5, 10, 15, m[10], m[14]);
        G(v, 1, 6, 11, 12, m[3], m[6]);
        G(v, 2, 7, 8, 13, m[7], m[1]);
        G(v, 3, 4, 9, 14, m[9], m[4]);

        G(v, 0, 4, 8, 12, m[7], m[9]);
        G(v, 1, 5, 9, 13, m[3], m[1]);
        G(v, 2, 6, 10, 14, m[13], m[12]);
        G(v, 3, 7, 11, 15, m[11], m[14]);
        G(v, 0, 5, 10, 15, m[2], m[6]);
        G(v, 1, 6, 11, 12, m[5], m[10]);
        G(v, 2, 7, 8, 13, m[4], m[0]);
        G(v, 3, 4, 9, 14, m[15], m[8]);

        G(v, 0, 4, 8, 12, m[9], m[0]);
        G(v, 1, 5, 9, 13, m[5], m[7]);
        G(v, 2, 6, 10, 14, m[2], m[4]);
        G(v, 3, 7, 11, 15, m[10], m[15]);
        G(v, 0, 5, 10, 15, m[14], m[1]);
        G(v, 1, 6, 11, 12, m[11], m[12]);
        G(v, 2, 7, 8, 13, m[6], m[8]);
        G(v, 3, 4, 9, 14, m[3], m[13]);

        G(v, 0, 4, 8, 12, m[2], m[12]);
        G(v, 1, 5, 9, 13, m[6], m[10]);
        G(v, 2, 6, 10, 14, m[0], m[11]);
        G(v, 3, 7, 11, 15, m[8], m[3]);
        G(v, 0, 5, 10, 15, m[4], m[13]);
        G(v, 1, 6, 11, 12, m[7], m[5]);
        G(v, 2, 7, 8, 13, m[15], m[14]);
        G(v, 3, 4, 9, 14, m[1], m[9]);

        G(v, 0, 4, 8, 12, m[12], m[5]);
        G(v, 1, 5, 9, 13, m[1], m[15]);
        G(v, 2, 6, 10, 14, m[14], m[13]);
        G(v, 3, 7, 11, 15, m[4], m[10]);
        G(v, 0, 5, 10, 15, m[0], m[7]);
        G(v, 1, 6, 11, 12, m[6], m[3]);
        G(v, 2, 7, 8, 13, m[9], m[2]);
        G(v, 3, 4, 9, 14, m[8], m[11]);

        G(v, 0, 4, 8, 12, m[13], m[11]);
        G(v, 1, 5, 9, 13, m[7], m[14]);
        G(v, 2, 6, 10, 14, m[12], m[1]);
        G(v, 3, 7, 11, 15, m[3], m[9]);
        G(v, 0, 5, 10, 15, m[5], m[0]);
        G(v, 1, 6, 11, 12, m[15], m[4]);
        G(v, 2, 7, 8, 13, m[8], m[6]);
        G(v, 3, 4, 9, 14, m[2], m[10]);

        G(v, 0, 4, 8, 12, m[6], m[15]);
        G(v, 1, 5, 9, 13, m[14], m[9]);
        G(v, 2, 6, 10, 14, m[11], m[3]);
        G(v, 3, 7, 11, 15, m[0], m[8]);
        G(v, 0, 5, 10, 15, m[12], m[2]);
        G(v, 1, 6, 11, 12, m[13], m[7]);
        G(v, 2, 7, 8, 13, m[1], m[4]);
        G(v, 3, 4, 9, 14, m[10], m[5]);

        G(v, 0, 4, 8, 12, m[10], m[2]);
        G(v, 1, 5, 9, 13, m[8], m[4]);
        G(v, 2, 6, 10, 14, m[7], m[6]);
        G(v, 3, 7, 11, 15, m[1], m[5]);
        G(v, 0, 5, 10, 15, m[15], m[11]);
        G(v, 1, 6, 11, 12, m[9], m[14]);
        G(v, 2, 7, 8, 13, m[3], m[12]);
        G(v, 3, 4, 9, 14, m[13], m[0]);

        G(v, 0, 4, 8, 12, m[0], m[1]);
        G(v, 1, 5, 9, 13, m[2], m[3]);
        G(v, 2, 6, 10, 14, m[4], m[5]);
        G(v, 3, 7, 11, 15, m[6], m[7]);
        G(v, 0, 5, 10, 15, m[8], m[9]);
        G(v, 1, 6, 11, 12, m[10], m[11]);
        G(v, 2, 7, 8, 13, m[12], m[13]);
        G(v, 3, 4, 9, 14, m[14], m[15]);

        G(v, 0, 4, 8, 12, m[14], m[10]);
        G(v, 1, 5, 9, 13, m[4], m[8]);
        G(v, 2, 6, 10, 14, m[9], m[15]);
        G(v, 3, 7, 11, 15, m[13], m[6]);
        G(v, 0, 5, 10, 15, m[1], m[12]);
        G(v, 1, 6, 11, 12, m[0], m[2]);
        G(v, 2, 7, 8, 13, m[11], m[7]);
        G(v, 3, 4, 9, 14, m[5], m[3]);

        //XOR current state with both halves of v
        for (uint256 i = 0; i < 8; ++i) {
            ctx.h[i] = ctx.h[i] ^ v[i] ^ v[i + 8];
        }
    }

    function init(
        BLAKE2b_ctx memory ctx,
        uint64 outlen,
        bytes memory key,
        uint64[2] memory salt,
        uint64[2] memory person
    ) internal {
        assert(!(outlen == 0 || outlen > 64 || key.length > 64));

        //Initialize chained-state to IV
        for (uint256 i = 0; i < 8; i++) {
            ctx.h[i] = IV[i];
        }

        // Set up parameter block
        ctx.h[0] =
            ctx.h[0] ^
            0x01010000 ^
            shift_left(uint64(key.length), 8) ^
            outlen;
        ctx.h[4] = ctx.h[4] ^ salt[0];
        ctx.h[5] = ctx.h[5] ^ salt[1];
        ctx.h[6] = ctx.h[6] ^ person[0];
        ctx.h[7] = ctx.h[7] ^ person[1];

        ctx.outlen = outlen;
        // TODO i is not used why?
        //i = key.length;

        //Run hash once with key as input
        if (key.length > 0) {
            update(ctx, key);
            ctx.c = 128;
        }
    }

    function update(BLAKE2b_ctx memory ctx, bytes memory input) internal {
        for (uint256 i = 0; i < input.length; i++) {
            //If buffer is full, update byte counters and compress
            if (ctx.c == 128) {
                ctx.t += ctx.c;
                compress(ctx, false);
                ctx.c = 0;
            }

            //Update temporary counter c
            uint256 c = ctx.c++;

            // b -> ctx.b
            uint256[4] memory b = ctx.b;
            uint8 a = uint8(input[i]);

            // ctx.b[c] = a
            assembly {
                mstore8(add(b, c), a)
            }
        }
    }

    function finalize(BLAKE2b_ctx memory ctx, uint64[8] memory out) internal {
        // Add any uncounted bytes
        ctx.t += ctx.c;

        // Compress with finalization flag
        compress(ctx, true);

        //Flip little to big endian and store in output buffer
        for (uint256 i = 0; i < ctx.outlen / 8; i++) {
            out[i] = getWords(ctx.h[i]);
        }

        //Properly pad output if it doesn't fill a full word
        if (ctx.outlen < 64) {
            out[ctx.outlen / 8] = shift_right(
                getWords(ctx.h[ctx.outlen / 8]),
                64 - 8 * (ctx.outlen % 8)
            );
        }
    }

    //Helper function for full hash function
    function blake2b_full(
        bytes memory input,
        bytes memory key,
        bytes memory salt,
        bytes memory personalization,
        uint64 outlen
    ) public returns (uint64[8] memory) {
        BLAKE2b_ctx memory ctx;
        uint64[8] memory out;

        init(ctx, outlen, key, formatInput(salt), formatInput(personalization));
        update(ctx, input);
        finalize(ctx, out);
        return out;
    }

    // TODO this function should not be here
    // TODO or maybe make this function more generic
    function blake2b_with_updates_branch(
        bytes memory persona,
        bytes calldata left,
        bytes calldata right
    ) public returns (uint64[8] memory) {
        BLAKE2b.BLAKE2b_ctx memory ctx;
        uint64[8] memory out;
        uint64 outlen = 64;

        init(ctx, outlen, "", formatInput(""), formatInput(persona));

        bytes memory l_tag = abi.encodePacked("l");
        console.log("l");
        console.logBytes(l_tag);
        bytes memory r_tag = abi.encodePacked("r");
        console.log("r");
        console.logBytes(r_tag);

        // update(ctx, l_tag); TODO: re-enable
        update(ctx, left);
        // update(ctx, r_tag); TODO: re-enable
        update(ctx, right);
        finalize(ctx, out);
        return out;
    }

    function blake2b(
        bytes memory input,
        bytes memory key,
        uint64 outlen
    ) public returns (uint64[8] memory) {
        return blake2b_full(input, key, "", "", outlen);
    }

    // Utility functions

    //https://ethereum.stackexchange.com/questions/83626/how-to-reverse-byte-order-in-uint256-or-bytes32
    function getWords(uint64 input) internal pure returns (uint64 v) {
        v = input;

        // swap bytes
        v = ((v & 0xFF00FF00FF00FF00) >> 8) | ((v & 0x00FF00FF00FF00FF) << 8);

        // swap 2-byte long pairs
        v = ((v & 0xFFFF0000FFFF0000) >> 16) | ((v & 0x0000FFFF0000FFFF) << 16);

        // swap 4-byte long pairs
        v = (v >> 32) | (v << 32);
    }

    function shift_right(uint64 a, uint256 shift) private returns (uint64) {
        return a >> shift;
    }

    function shift_left(uint64 a, uint256 shift) private returns (uint64) {
        return a << shift;
    }

    //bytes -> uint64[2]
    function formatInput(bytes memory input)
        public
        returns (uint64[2] memory output)
    {
        for (uint256 i = 0; i < input.length; i++) {
            // TODO clean?
            bytes memory slice = new bytes(8);
            slice[7] = input[i];
            uint64 v = BytesLib.toUint64(slice, 0);

            output[i / 8] =
                output[i / 8] ^
                shift_left(v, 64 - 8 * ((i % 8) + 1));
        }
        output[0] = getWords(output[0]);
        output[1] = getWords(output[1]);
    }

    function formatOutput(uint64[8] memory input)
        public
        returns (bytes32[2] memory)
    {
        bytes32[2] memory result;

        for (uint256 i = 0; i < 8; i++) {
            result[i / 4] =
                result[i / 4] ^
                bytes32(input[i] * 2**(64 * (3 - (i % 4))));
        }
        return result;
    }
}
