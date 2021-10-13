// SPDX-License-Identifier: MIT
// Borrowed from https://github.com/nervosnetwork/force-bridge-eth
pragma solidity ^0.8.0;
pragma abicoder v2;

library CKBCrypto {
    struct Instance {
        // This is a bit misleadingly called state as it not only includes the Blake2 state,
        // but every field needed for the "blake2 f function precompile".
        //
        // This is a tightly packed buffer of:
        // - rounds: 32-bit BE
        // - h: 8 x 64-bit LE
        // - m: 16 x 64-bit LE
        // - t: 2 x 64-bit LE
        // - f: 8-bit
        bytes state;
        // Expected output hash length. (Used in `finalize`.)
        uint256 out_len;
        // Data passed to "function F".
        // NOTE: this is limited to 24 bits.
        uint256 input_counter;
    }

    // Initialise the state with a given `key` and required `out_len` hash length.
    function init() internal pure returns (Instance memory instance) {
        // Safety check that the precompile exists.
        // TODO: remove this?
        //         assembly {
        //            if eq(extcodehash(0x09), 0) { revert(0, 0) }
        //        }

        instance.out_len = 32;
        instance.input_counter = 0;
        instance
            .state = hex"0000000c28c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b08d623d6cfbce57e0c4d0a3e71ac933300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    }

    // This calls the blake2 precompile ("function F of the spec").
    // It expects the state was updated with the next block. Upon returning the state will be updated,
    // but the supplied block data will not be cleared.
    function call_function_f(Instance memory instance) private view {
        bytes memory state = instance.state;
        assembly {
            let state_ptr := add(state, 32)
            if iszero(
                staticcall(
                    not(0),
                    0x09,
                    state_ptr,
                    0xd5,
                    add(state_ptr, 4),
                    0x40
                )
            ) {
                revert(0, 0)
            }
        }
    }

    // This function will split blocks correctly and repeatedly call the precompile.
    // NOTE: this is dumb right now and expects `data` to be 128 bytes long and padded with zeroes,
    //       hence the real length is indicated with `data_len`
    function update_loop(
        Instance memory instance,
        bytes memory data,
        uint256 data_len,
        bool last_block
    ) private view {
        bytes memory state = instance.state;
        uint256 input_counter = instance.input_counter;

        // This is the memory location where the "data block" starts for the precompile.
        uint256 state_ptr;
        assembly {
            // The `rounds` field is 4 bytes long and the `h` field is 64-bytes long.
            // Also adjust for the size of the bytes type.
            state_ptr := add(state, 100)
        }

        // This is the memory location where the input data resides.
        uint256 data_ptr;
        assembly {
            data_ptr := add(data, 32)
        }

        uint256 len = data.length;
        while (len > 0) {
            if (len >= 128) {
                assembly {
                    mstore(state_ptr, mload(data_ptr))
                    data_ptr := add(data_ptr, 32)

                    mstore(add(state_ptr, 32), mload(data_ptr))
                    data_ptr := add(data_ptr, 32)

                    mstore(add(state_ptr, 64), mload(data_ptr))
                    data_ptr := add(data_ptr, 32)

                    mstore(add(state_ptr, 96), mload(data_ptr))
                    data_ptr := add(data_ptr, 32)
                }

                len -= 128;
                // FIXME: remove this once implemented proper padding
                if (data_len < 128) {
                    input_counter += data_len;
                } else {
                    data_len -= 128;
                    input_counter += 128;
                }
            } else {
                // FIXME: implement support for smaller than 128 byte blocks
                revert();
            }

            // Set length field (little-endian) for maximum of 24-bits.
            assembly {
                mstore8(add(state, 228), and(input_counter, 0xff))
                mstore8(add(state, 229), and(shr(8, input_counter), 0xff))
                mstore8(add(state, 230), and(shr(16, input_counter), 0xff))
            }

            // Set the last block indicator.
            // Only if we've processed all input.
            if (len == 0) {
                assembly {
                    // Writing byte 212 here.
                    mstore8(add(state, 244), last_block)
                }
            }

            // Call the precompile
            call_function_f(instance);
        }

        instance.input_counter = input_counter;
    }

    // Update the state with a non-final block.
    // NOTE: the input must be complete blocks.
    function update(
        Instance memory instance,
        bytes memory data,
        uint256 data_len
    ) internal view {
        require((data.length % 128) == 0);
        update_loop(instance, data, data_len, false);
    }

    // Update the state with a final block and return the hash.
    function finalize(
        Instance memory instance,
        bytes memory data,
        uint256 data_len
    ) internal view returns (bytes32 output) {
        // FIXME: support incomplete blocks (zero pad them)
        // assert((data.length % 128) == 0);
        uint256 remainder = data.length % 128;
        if (0 != remainder) {
            bytes memory fixed_data = abi.encodePacked(
                data,
                new bytes(128 - remainder)
            );
            update_loop(instance, fixed_data, data_len, true);
        } else {
            update_loop(instance, data, data_len, true);
        }

        bytes memory state = instance.state;
        assembly {
            output := mload(add(state, 36))
        }
    }

    // TODO optimize blake2b
    // 1. optimize memory alloc
    // 2. optimize input data length adaption
    function digest(bytes memory data, uint256 data_len)
        internal
        view
        returns (bytes32 output)
    {
        Instance memory instance = init();
        output = finalize(instance, data, data_len);
    }
}
