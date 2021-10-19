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

    // TODO should this be removed?
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
        // rounds - the number of rounds - 32-bit unsigned big-endian word
        //
        // h - the state vector - 8 unsigned 64-bit little-endian words
        // m - the message block vector - 16 unsigned 64-bit little-endian words
        // t_0, t_1 - offset counters - 2 unsigned 64-bit little-endian words
        //
        // f - the final block indicator flag - 8-bit word
        // [4 bytes for rounds][64 bytes for h][128 bytes for m][8 bytes for t_0][8 bytes for t_1][1 byte for f]

        // Prepare call to precompiled function F
        uint32 rounds = 12; // turned into big endian by abi.encodePacked
        bytes32[2] memory h = Uint64Array8ToBytes32Array2(ctx.h); // Should be ok
        bytes32[4] memory m = convert_buffer_to_message(ctx.b); // Convert From 4 256bits to 16 64 bits
        bytes8[2] memory t = Uint128ToBytes8(ctx.t); // Maybe it is ok
        bool f = last; // Should be ok

        // console.log("is last %s ", last);
        // console.logBytes32(m[0]);
        // console.log(ctx.b[0]);
        // console.logBytes32(m[1]);
        // console.log(ctx.b[1]);
        // console.logBytes32(m[2]);
        // console.log(ctx.b[2]);
        // console.logBytes32(m[3]);
        // console.log(ctx.b[3]);
        // console.log("offset");
        // console.logBytes8(t[0]);
        // console.logBytes8(t[1]);
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

    function Uint64Array8ToBytes32Array2(uint64[8] memory arr)
        public
        pure
        returns (bytes32[2] memory c)
    {
        c[0] = BytesLib.toBytes32(
            abi.encodePacked(
                reverse64(arr[0]),
                reverse64(arr[1]),
                reverse64(arr[2]),
                reverse64(arr[3])
            ),
            0
        );
        c[1] = BytesLib.toBytes32(
            abi.encodePacked(
                reverse64(arr[4]),
                reverse64(arr[5]),
                reverse64(arr[6]),
                reverse64(arr[7])
            ),
            0
        );
    }

    function Uint64Array16ToBytes32Array4(uint64[16] memory arr)
        public
        pure
        returns (bytes32[4] memory c)
    {
        c[0] = BytesLib.toBytes32(
            abi.encodePacked(
                reverse64(arr[0]),
                reverse64(arr[1]),
                reverse64(arr[2]),
                reverse64(arr[3])
            ),
            0
        );
        c[1] = BytesLib.toBytes32(
            abi.encodePacked(
                reverse64(arr[4]),
                reverse64(arr[5]),
                reverse64(arr[6]),
                reverse64(arr[7])
            ),
            0
        );
        c[2] = BytesLib.toBytes32(
            abi.encodePacked(
                reverse64(arr[8]),
                reverse64(arr[9]),
                reverse64(arr[10]),
                reverse64(arr[11])
            ),
            0
        );
        c[3] = BytesLib.toBytes32(
            abi.encodePacked(
                reverse64(arr[12]),
                reverse64(arr[13]),
                reverse64(arr[14]),
                reverse64(arr[15])
            ),
            0
        );
    }

    // m - the message block vector - 16 unsigned 64-bit little-endian words
    //    function Uint256ArrayToBytesArray(uint256[4] memory arr)
    //        public
    //        view
    //        returns (bytes32[4] memory c)
    //    {
    //        for (uint256 i = 0; i < 4; i++) {
    //            uint256 x = arr[i];
    //
    //            c[i] = BytesLib.toBytes32(
    //                abi.encodePacked(
    //                    reverse64(uint64(x >> 196)),
    //                    reverse64(uint64(x >> 128)),
    //                    reverse64(uint64(x >> 64)),
    //                    reverse64(uint64(x >> 0))
    //                ),
    //                0
    //            );
    //        }
    //    }

    function Uint128ToBytes8(uint128 t)
        public
        pure
        returns (bytes8[2] memory c)
    {
        c[0] = bytes8(reverse64(uint64(t >> 64)));
        c[1] = bytes8(reverse64(uint64(t)));
    }

    function Bytes32ArrayToUint64Array(bytes32[2] memory arr)
        public
        pure
        returns (uint64[8] memory c)
    {
        bytes memory as_bytes = abi.encodePacked(arr);
        for (uint256 i = 0; i < 8; i++) {
            // TODO do we need to revers the endianness here too?
            c[i] = BytesLib.toUint64(as_bytes, 8 * i);
        }
    }

    // The endian reversal function are from
    //     https://ethereum.stackexchange.com/a/83627
    // overloading with the argument types did not work as expected, hence the many names.

    function reverse256(uint256 input) internal pure returns (uint256 v) {
        v = input;

        // swap bytes
        v =
            ((v &
                0xFF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00) >>
                8) |
            ((v &
                0x00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF) <<
                8);

        // swap 2-byte long pairs
        v =
            ((v &
                0xFFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000) >>
                16) |
            ((v &
                0x0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF) <<
                16);

        // swap 4-byte long pairs
        v =
            ((v &
                0xFFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000) >>
                32) |
            ((v &
                0x00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF) <<
                32);

        // swap 8-byte long pairs
        v =
            ((v &
                0xFFFFFFFFFFFFFFFF0000000000000000FFFFFFFFFFFFFFFF0000000000000000) >>
                64) |
            ((v &
                0x0000000000000000FFFFFFFFFFFFFFFF0000000000000000FFFFFFFFFFFFFFFF) <<
                64);

        // swap 16-byte long pairs
        v = (v >> 128) | (v << 128);
    }

    function reverse128(uint128 input) internal pure returns (uint128 v) {
        v = input;

        // swap bytes
        v =
            ((v & 0xFF00FF00FF00FF00FF00FF00FF00FF00) >> 8) |
            ((v & 0x00FF00FF00FF00FF00FF00FF00FF00FF) << 8);

        // swap 2-byte long pairs
        v =
            ((v & 0xFFFF0000FFFF0000FFFF0000FFFF0000) >> 16) |
            ((v & 0x0000FFFF0000FFFF0000FFFF0000FFFF) << 16);

        // swap 4-byte long pairs
        v =
            ((v & 0xFFFFFFFF00000000FFFFFFFF00000000) >> 32) |
            ((v & 0x00000000FFFFFFFF00000000FFFFFFFF) << 32);

        // swap 8-byte long pairs
        v = (v >> 64) | (v << 64);
    }

    function reverse64(uint64 input) internal pure returns (uint64 v) {
        v = input;

        // swap bytes
        v = ((v & 0xFF00FF00FF00FF00) >> 8) | ((v & 0x00FF00FF00FF00FF) << 8);

        // swap 2-byte long pairs
        v = ((v & 0xFFFF0000FFFF0000) >> 16) | ((v & 0x0000FFFF0000FFFF) << 16);

        // swap 4-byte long pairs
        v = (v >> 32) | (v << 32);
    }

    function reverse64Array(uint64[8] memory input)
        internal
        pure
        returns (uint64[8] memory out)
    {
        for (uint256 i = 0; i < input.length; i++) {
            out[i] = reverse64(input[i]);
        }
    }

    function reverse128Array(uint128[2] memory input)
        internal
        pure
        returns (uint128[2] memory out)
    {
        for (uint256 i = 0; i < input.length; i++) {
            out[i] = reverse128(input[i]);
        }
    }

    function reverse256Array(uint256[4] memory input)
        internal
        pure
        returns (uint256[4] memory out)
    {
        for (uint256 i = 0; i < input.length; i++) {
            out[i] = reverse256(input[i]);
        }
    }

    // just for checking during development
    function endian(uint256 input) public view returns (bytes32 c) {
        return bytes32(reverse256(input));
    }

    // just for checking during development
    function encodePacked(uint256 input) public view returns (bytes memory) {
        return abi.encodePacked(input);
    }

    /// Implements the conversion from buffer `b` to message `m` of the compression function
    /// between lines https://github.com/ConsenSys/Project-Alchemy/blob/9812c33c24a49448660d4a2d226caa80ac982102/contracts/BLAKE2b/BLAKE2b.sol#L71
    /// and https://github.com/ConsenSys/Project-Alchemy/blob/9812c33c24a49448660d4a2d226caa80ac982102/contracts/BLAKE2b/BLAKE2b.sol#L85
    function convert_buffer_to_message(uint256[4] memory b_arr)
        public
        returns (bytes32[4] memory c)
    {
        uint64[16] memory m;
        uint64 mi; //Temporary stack variable to decrease memory ops
        uint256 b; // Input buffer

        for (uint256 i = 0; i < 16; i++) {
            //Operate 16 words at a time
            uint256 k = i % 4; //Current buffer word
            mi = 0;
            if (k == 0) {
                b = b_arr[i / 4]; //Load relevant input into buffer
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
        return Uint64Array16ToBytes32Array4(m);
    }
}
