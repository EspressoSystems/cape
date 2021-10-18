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

    // From https://eips.ethereum.org/EIPS/eip-152
    function F(
        uint32 rounds,
        bytes32[2] memory h,
        bytes32[4] memory m,
        bytes8[2] memory t,
        bool f
    ) public view returns (bytes32[2] memory) {
        bytes32[2] memory output;

        bytes memory args = abi.encodePacked(
            rounds,
            h[0],
            h[1],
            m[0],
            m[1],
            m[2],
            m[3],
            t[0],
            t[1],
            f
        );

        assembly {
            if iszero(
                staticcall(not(0), 0x09, add(args, 32), 0xd5, output, 0x40)
            ) {
                revert(0, 0)
            }
        }

        return output;
    }

    function compress(BLAKE2b_ctx memory ctx, bool last) internal {
        // Prepare call to precompiled function F
        uint32 rounds = 12;
        bytes32[2] memory h = Uint64ArrayToBytes32Array(ctx.h); // Should be ok
        bytes32[4] memory m = Uint256ArrayToBytesArray(ctx.b); // Convert From 4 256bits to 16 64 bits
        bytes8[2] memory t = Uint128ToBytes8(ctx.t); // Maybe it is ok
        bool f = last; // Should be ok

        // Call precompiled function F
        ctx.h = Bytes32ArrayToUint64Array(F(rounds, h, m, t, f));
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

    //Flips endianness of words
    function getWords(uint64 a) private returns (uint64 b) {
        return
            ((a & MASK_0) / SHIFT_0) ^
            ((a & MASK_1) / SHIFT_1) ^
            ((a & MASK_2) / SHIFT_2) ^
            ((a & MASK_3) / SHIFT_3) ^
            ((a & MASK_4) * SHIFT_3) ^
            ((a & MASK_5) * SHIFT_2) ^
            ((a & MASK_6) * SHIFT_1) ^
            ((a & MASK_7) * SHIFT_0);
    }

    function shift_right(uint64 a, uint256 shift) private returns (uint64 b) {
        return uint64(a / 2**shift);
    }

    function shift_left(uint64 a, uint256 shift) private returns (uint64) {
        return uint64((a * 2**shift) % (2**64));
    }

    //bytes -> uint64[2]
    function formatInput(bytes memory input)
        private
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
        private
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

    ///////////////////////////////////////////////////
    // Helper functions to be moved to another file
    // However by doing so it seems to consume more gas
    //////////////////////////////////////////////////

    function Uint64ToBytes(uint64 x) public pure returns (bytes memory c) {
        bytes8 b = bytes8(x);
        c = new bytes(8);
        for (uint256 i = 0; i < 8; i++) {
            c[i] = b[i];
        }
    }

    function Uint64ArrayToBytes32Array(uint64[8] memory arr)
        public
        pure
        returns (bytes32[2] memory c)
    {
        bytes32[2] memory c;
        return c;
    }

    function Uint256ArrayToBytesArray(uint256[4] memory arr)
        public
        pure
        returns (bytes32[4] memory c)
    {
        bytes32[4] memory c;
        return c;
    }

    function Uint128ToBytes8(uint128 t)
        public
        pure
        returns (bytes8[2] memory c)
    {
        bytes8[2] memory c;
        return c;
    }

    function Bytes32ArrayToUint64Array(bytes32[2] memory arr)
        public
        pure
        returns (uint64[8] memory c)
    {
        uint64[8] memory c;
        return c;
    }
}
