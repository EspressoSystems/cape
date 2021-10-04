// SPDX-License-Identifier: MIT
// Borrowed from https://github.com/nervosnetwork/force-bridge-eth
pragma solidity ^0.8.0;
pragma abicoder v2;

library  Blake2b {

    function digest208(bytes memory input208) internal view returns (bytes32 ret){
        // solium-disable-next-line
        assembly{

        // reject not ckbinput
        // 208 = 0xD0
            if iszero(eq(mload(input208), 0xD0)){
                revert(0x80,0x00)
            }

            let input_ptr := add(input208,0x20)
        // the init vector is 0x
        //  08c9bc3f 67e6096a 3ba7ca84 85ae67bb
        //  2bf894fe 72f36e3c f1361d5f 3af54fa5
        //  d182e6ad 7f520e51 1f6c3e2b 8c68059b
        //  6bbd41fb abd9831f 79217e13 19cde05b
        //
        // the param is
        //         let param = blake2b_param {
        //            digest_length: out_len as u8,   u8 0x20
        //            key_length: 0, u8 0x00
        //            fanout: 1, u8 0x01
        //            depth: 1,  u8 0x01
        //            leaf_length: 0, u32
        //            node_offset: 0,u32
        //            xof_length: 0, u32
        //            node_depth: 0, u8
        //            inner_length: 0, u8
        //            reserved: [0u8; 14usize], u8[14]
        //            salt: [0u8; blake2b_constant_BLAKE2B_SALTBYTES as usize], u8[16]
        //            personal: [0u8; blake2b_constant_BLAKE2B_PERSONALBYTES as usize], u8[16]
        //        };
        // pub const blake2b_constant_BLAKE2B_SALTBYTES: blake2b_constant = 16;
        // pub const blake2b_constant_BLAKE2B_PERSONALBYTES: blake2b_constant = 16;
        // the PERSONALBYTES is b"ckb-default-hash";
        // PERSONALBYTES = 636b622d 64656661 756c742d 68617368
        //
        // the param is 0x
        // 20000101 00000000 00000000 00000000 [digest_length key_length fanout depth] leaf_length node_offset xof_length
        // 00000000 00000000 00000000 00000000 node_depth inner_length reserved
        // 00000000 00000000 00000000 00000000 salt
        // 636b622d 64656661 756c742d 68617368 personal
        //
        // iv ^ param is 64 bytes, which is the init h
        // 28c9bdf2 67e6096a 3ba7ca84 85ae67bb
        // 2bf894fe 72f36e3c f1361d5f 3af54fa5
        // d182e6ad 7f520e51 1f6c3e2b 8c68059b
        // 08d623d6 cfbce57e 0c4d0a3e 71ac9333
        //
        // param for blake2b F():
        // rounds - the number of rounds - 32-bit unsigned big-endian word
        // h - the state vector - 8 unsigned 64-bit little-endian words
        // m - the message block vector - 16 unsigned 64-bit little-endian words
        // t_0, t_1 - offset counters - 2 unsigned 64-bit little-endian words
        // f - the final block indicator flag - 8-bit word
        //
        // the rounds === 12 for blake2b, 10 for blake2s
        // h is state vector, the first/init is iv^param as above
        // m, t_0, t_1 and f is initialized as 0
        // the first call param is
        // 0000000c28c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b08d623d6cfbce57e0c4d0a3e71ac933300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
        //
        // we don't have private/password 'key', so we can skip 'first' update loop
        //
        // 208 = 128 + 80 + 28 padding
        // it need 7slot * 32(256bits) = 224 bytes to cover m

        // due to call/staticcall/delegagecall, we have to use memory
        // 0x80 + 213 = 341 0x0155
        // 0x80 + 0xe0 = 0x0160, we align memory to 0x20 bytes due to evm is 256-bit machine
        // take over memory management

            let memory_ptr := mload(0x40)
        // allocate more 0xE0 bytes temporarily
            mstore(0x40, add(memory_ptr,0x0E0))

        // total size = E0
        //   offset
        //        round   h
        //   0x00 0000000c28c9bdf267e6096a3ba7ca84
        //   0x10 85ae67bb2bf894fe72f36e3cf1361d5f

        //   0x20 3af54fa5d182e6ad7f520e511f6c3e2b
        //   0x30 8c68059b08d623d6cfbce57e0c4d0a3e

        //                m
        //   0x40 71ac9333000000000000000000000000
        //   0x50 00000000000000000000000000000000

        //   0x60 00000000000000000000000000000000
        //   0x70 00000000000000000000000000000000

        //   0x80 00000000000000000000000000000000
        //   0x90 00000000000000000000000000000000

        //   0xA0 00000000000000000000000000000000
        //   0xB0 00000000000000000000000000000000
        //
        //                t0              t1
        //   0xC0 00000000000000000000000000000000
        //   0xD0 0000000000PPPPPPPPPPPPPPPPPPPPPP  P for placeholder
        //                fi
        // populate init param
            mstore(memory_ptr, 0x0000000c28c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f)
            mstore(add(memory_ptr,0x20), 0x3af54fa5d182e6ad7f520e511f6c3e2b8c68059b08d623d6cfbce57e0c4d0a3e)
            mstore(add(memory_ptr,0x40), 0x71ac933300000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0x60), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0x80), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0xA0), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0xC0), 0x0000000000000000000000000000000000000000000000000000000000000000)


        /*// copy 128 0x80 bytes to m, eliminate selector and leading length of bytes
            calldatacopy(0xC4, 0x44, 0x80)*/

        //copy 0x80 bytes in memory from input_ptr to

        // 0x00
            mstore(add(memory_ptr,0x44),mload(input_ptr))
        // 0x20
            mstore(add(memory_ptr,0x64),mload(add(input_ptr,0x20)))
        // 0x40
            mstore(add(memory_ptr,0x84),mload(add(input_ptr,0x40)))
        // 0x60
            mstore(add(memory_ptr,0xA4),mload(add(input_ptr,0x60)))

        // set t0,t1 to 128 0x80
        // watch that the data is Little.Endian
        // t0 = 0x 80 00 00 00 00 00 00 00
        // t1 = 0x 00 00 00 00 00 00 00 00
            mstore8(add(memory_ptr,0xC4),0x80)

        // not final block, leave f to 0x00

        // call F()

        // pass memory to blake2b, get the result h at 0x80+0x04, over-writing
            if iszero(staticcall(not(0), 0x09, memory_ptr, 0xD5, add(memory_ptr,0x04), 0x40)) {
                revert(0x80, 0x00)
            }

        // the remaining 208-128=80 0x50 bytes input data

        // copy 208-128=80 0x50 bytes to m, need padding zero

        // 0x80
            mstore(add(memory_ptr,0x44),mload(add(input_ptr,0x80)))
        // 0xA0
            mstore(add(memory_ptr,0x64),mload(add(input_ptr,0xA0)))
        // 0xC0, the data size is 0xD0, we must truncate the low bytes
            mstore(add(memory_ptr,0x84),and(mload(add(input_ptr,0xC0)),0xffffffffffffffffffffffffffffffff00000000000000000000000000000000))
        // 0xE0
            mstore(add(memory_ptr,0xA4),0x0000000000000000000000000000000000000000000000000000000000000000)


        // set t0,t1 to 208 0xD0
        // watch that the data is Little.Endian
        // t0 = 0x D0 00 00 00 00 00 00 00
        // t1 = 0x 00 00 00 00 00 00 00 00
            mstore8(add(memory_ptr,0xC4),0xD0)

        // final block, set f to 0x01
            mstore8(add(memory_ptr,0xD4),0x01)

        // call F()
        // pass memory to blake2b, get the result h at 0x80+0x04, over-writing
            if iszero(staticcall(not(0), 0x09, memory_ptr, 0xD5, add(memory_ptr,0x04), 0x40)) {
                revert(0x80, 0x00)
            }

            ret :=  mload(add(memory_ptr,0x04))
        // clear memory
            mstore(0x40,memory_ptr)
        }
    }

    function digest208Ptr(uint256 input_ptr) internal view returns (bytes32 ret){
        // solium-disable-next-line
        assembly{
        // the init vector is 0x
        //  08c9bc3f 67e6096a 3ba7ca84 85ae67bb
        //  2bf894fe 72f36e3c f1361d5f 3af54fa5
        //  d182e6ad 7f520e51 1f6c3e2b 8c68059b
        //  6bbd41fb abd9831f 79217e13 19cde05b
        //
        // the param is
        //         let param = blake2b_param {
        //            digest_length: out_len as u8,   u8 0x20
        //            key_length: 0, u8 0x00
        //            fanout: 1, u8 0x01
        //            depth: 1,  u8 0x01
        //            leaf_length: 0, u32
        //            node_offset: 0,u32
        //            xof_length: 0, u32
        //            node_depth: 0, u8
        //            inner_length: 0, u8
        //            reserved: [0u8; 14usize], u8[14]
        //            salt: [0u8; blake2b_constant_BLAKE2B_SALTBYTES as usize], u8[16]
        //            personal: [0u8; blake2b_constant_BLAKE2B_PERSONALBYTES as usize], u8[16]
        //        };
        // pub const blake2b_constant_BLAKE2B_SALTBYTES: blake2b_constant = 16;
        // pub const blake2b_constant_BLAKE2B_PERSONALBYTES: blake2b_constant = 16;
        // the PERSONALBYTES is b"ckb-default-hash";
        // PERSONALBYTES = 636b622d 64656661 756c742d 68617368
        //
        // the param is 0x
        // 20000101 00000000 00000000 00000000 [digest_length key_length fanout depth] leaf_length node_offset xof_length
        // 00000000 00000000 00000000 00000000 node_depth inner_length reserved
        // 00000000 00000000 00000000 00000000 salt
        // 636b622d 64656661 756c742d 68617368 personal
        //
        // iv ^ param is 64 bytes, which is the init h
        // 28c9bdf2 67e6096a 3ba7ca84 85ae67bb
        // 2bf894fe 72f36e3c f1361d5f 3af54fa5
        // d182e6ad 7f520e51 1f6c3e2b 8c68059b
        // 08d623d6 cfbce57e 0c4d0a3e 71ac9333
        //
        // param for blake2b F():
        // rounds - the number of rounds - 32-bit unsigned big-endian word
        // h - the state vector - 8 unsigned 64-bit little-endian words
        // m - the message block vector - 16 unsigned 64-bit little-endian words
        // t_0, t_1 - offset counters - 2 unsigned 64-bit little-endian words
        // f - the final block indicator flag - 8-bit word
        //
        // the rounds === 12 for blake2b, 10 for blake2s
        // h is state vector, the first/init is iv^param as above
        // m, t_0, t_1 and f is initialized as 0
        // the first call param is
        // 0000000c28c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b08d623d6cfbce57e0c4d0a3e71ac933300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
        //
        // we don't have private/password 'key', so we can skip 'first' update loop
        //
        // 208 = 128 + 80 + 28 padding
        // it need 7slot * 32(256bits) = 224 bytes to cover m

        // due to call/staticcall/delegagecall, we have to use memory
        // 0x80 + 213 = 341 0x0155
        // 0x80 + 0xe0 = 0x0160, we align memory to 0x20 bytes due to evm is 256-bit machine
        // take over memory management

            let memory_ptr := mload(0x40)
        // allocate more 0xE0 bytes temporarily
            mstore(0x40, add(memory_ptr,0x0E0))

        // total size = E0
        //   offset
        //        round   h
        //   0x00 0000000c28c9bdf267e6096a3ba7ca84
        //   0x10 85ae67bb2bf894fe72f36e3cf1361d5f

        //   0x20 3af54fa5d182e6ad7f520e511f6c3e2b
        //   0x30 8c68059b08d623d6cfbce57e0c4d0a3e

        //                m
        //   0x40 71ac9333000000000000000000000000
        //   0x50 00000000000000000000000000000000

        //   0x60 00000000000000000000000000000000
        //   0x70 00000000000000000000000000000000

        //   0x80 00000000000000000000000000000000
        //   0x90 00000000000000000000000000000000

        //   0xA0 00000000000000000000000000000000
        //   0xB0 00000000000000000000000000000000
        //
        //                t0              t1
        //   0xC0 00000000000000000000000000000000
        //   0xD0 0000000000PPPPPPPPPPPPPPPPPPPPPP  P for placeholder
        //                fi
        // populate init param
            mstore(memory_ptr, 0x0000000c28c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f)
            mstore(add(memory_ptr,0x20), 0x3af54fa5d182e6ad7f520e511f6c3e2b8c68059b08d623d6cfbce57e0c4d0a3e)
            mstore(add(memory_ptr,0x40), 0x71ac933300000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0x60), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0x80), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0xA0), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0xC0), 0x0000000000000000000000000000000000000000000000000000000000000000)


        /*// copy 128 0x80 bytes to m, eliminate selector and leading length of bytes
            calldatacopy(0xC4, 0x44, 0x80)*/

        //copy 0x80 bytes in memory from input_ptr to

        // 0x00
            mstore(add(memory_ptr,0x44),mload(input_ptr))
        // 0x20
            mstore(add(memory_ptr,0x64),mload(add(input_ptr,0x20)))
        // 0x40
            mstore(add(memory_ptr,0x84),mload(add(input_ptr,0x40)))
        // 0x60
            mstore(add(memory_ptr,0xA4),mload(add(input_ptr,0x60)))

        // set t0,t1 to 128 0x80
        // watch that the data is Little.Endian
        // t0 = 0x 80 00 00 00 00 00 00 00
        // t1 = 0x 00 00 00 00 00 00 00 00
            mstore8(add(memory_ptr,0xC4),0x80)

        // not final block, leave f to 0x00

        // call F()

        // pass memory to blake2b, get the result h at 0x80+0x04, over-writing
            if iszero(staticcall(not(0), 0x09, memory_ptr, 0xD5, add(memory_ptr,0x04), 0x40)) {
                revert(0x80, 0x00)
            }

        // the remaining 208-128=80 0x50 bytes input data

        // copy 208-128=80 0x50 bytes to m, need padding zero

        // 0x80
            mstore(add(memory_ptr,0x44),mload(add(input_ptr,0x80)))
        // 0xA0
            mstore(add(memory_ptr,0x64),mload(add(input_ptr,0xA0)))
        // 0xC0, the data size is 0xD0, we must truncate the low bytes
            mstore(add(memory_ptr,0x84),and(mload(add(input_ptr,0xC0)),0xffffffffffffffffffffffffffffffff00000000000000000000000000000000))
        // 0xE0
            mstore(add(memory_ptr,0xA4),0x0000000000000000000000000000000000000000000000000000000000000000)


        // set t0,t1 to 208 0xD0
        // watch that the data is Little.Endian
        // t0 = 0x D0 00 00 00 00 00 00 00
        // t1 = 0x 00 00 00 00 00 00 00 00
            mstore8(add(memory_ptr,0xC4),0xD0)

        // final block, set f to 0x01
            mstore8(add(memory_ptr,0xD4),0x01)

        // call F()
        // pass memory to blake2b, get the result h at 0x80+0x04, over-writing
            if iszero(staticcall(not(0), 0x09, memory_ptr, 0xD5, add(memory_ptr,0x04), 0x40)) {
                revert(0x80, 0x00)
            }

            ret :=  mload(add(memory_ptr,0x04))
        // clear memory
            mstore(0x40,memory_ptr)
        }
    }

    function digest64(bytes memory input64) internal view returns (bytes32 ret){
        // solium-disable-next-line
        assembly{
        // reject not ckbinput
        // 64 = 0x40
            if iszero(eq(mload(input64), 0x40)){
                revert(0x80,0x00)
            }

            let input_ptr := add(input64,0x20)

            let memory_ptr := mload(0x40)

            mstore(0x40, add(memory_ptr,0x0E0))

            mstore(memory_ptr, 0x0000000c28c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f)
            mstore(add(memory_ptr,0x20), 0x3af54fa5d182e6ad7f520e511f6c3e2b8c68059b08d623d6cfbce57e0c4d0a3e)
            mstore(add(memory_ptr,0x40), 0x71ac933300000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0x60), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0x80), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0xA0), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0xC0), 0x0000000000000000000000000000000000000000000000000000000000000000)

        //copy 0x40 bytes in memory from input_ptr to

        // 0x00
            mstore(add(memory_ptr,0x44),mload(input_ptr))
        // 0x20
            mstore(add(memory_ptr,0x64),mload(add(input_ptr,0x20)))
        // 0x40 set to 0x00
            mstore(add(memory_ptr,0x84),0x0000000000000000000000000000000000000000000000000000000000000000)
        // 0x60 set to 0x00
            mstore(add(memory_ptr,0xa4),0x0000000000000000000000000000000000000000000000000000000000000000)

        // set t0,t1 to 128 0x80
        // watch that the data is Little.Endian
        // t0 = 0x 80 00 00 00 00 00 00 00
        // t1 = 0x 00 00 00 00 00 00 00 00
            mstore8(add(memory_ptr,0xC4),0x40)

        // final block, set f to 0x01
            mstore8(add(memory_ptr,0xD4),0x01)

        // call F()
        // pass memory to blake2b, get the result h at 0x80+0x04, over-writing
            if iszero(staticcall(not(0), 0x09, memory_ptr, 0xD5, add(memory_ptr,0x04), 0x40)) {
                revert(0x80, 0x00)
            }

            ret :=  mload(add(memory_ptr,0x04))
        // clear memory
            mstore(0x40,memory_ptr)
        }
    }

    function digest64Merge(bytes32 left, bytes32 right) internal view returns (bytes32 ret){
        // solium-disable-next-line
        assembly{
            let memory_ptr := mload(0x40)

            mstore(0x40, add(memory_ptr,0x0E0))

            mstore(memory_ptr, 0x0000000c28c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f)
            mstore(add(memory_ptr,0x20), 0x3af54fa5d182e6ad7f520e511f6c3e2b8c68059b08d623d6cfbce57e0c4d0a3e)
            mstore(add(memory_ptr,0x40), 0x71ac933300000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0x60), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0x80), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0xA0), 0x0000000000000000000000000000000000000000000000000000000000000000)
            mstore(add(memory_ptr,0xC0), 0x0000000000000000000000000000000000000000000000000000000000000000)

        //copy 0x40 bytes in memory from input_ptr to

        // 0x00
            mstore(add(memory_ptr,0x44), left)
        // 0x20
            mstore(add(memory_ptr,0x64), right)
        // 0x40 set to 0x00
            mstore(add(memory_ptr,0x84),0x0000000000000000000000000000000000000000000000000000000000000000)
        // 0x60 set to 0x00
            mstore(add(memory_ptr,0xa4),0x0000000000000000000000000000000000000000000000000000000000000000)

        // set t0,t1 to 128 0x80
        // watch that the data is Little.Endian
        // t0 = 0x 80 00 00 00 00 00 00 00
        // t1 = 0x 00 00 00 00 00 00 00 00
            mstore8(add(memory_ptr,0xC4),0x40)

        // final block, set f to 0x01
            mstore8(add(memory_ptr,0xD4),0x01)

        // call F()
        // pass memory to blake2b, get the result h at 0x80+0x04, over-writing
            if iszero(staticcall(not(0), 0x09, memory_ptr, 0xD5, add(memory_ptr,0x04), 0x40)) {
                revert(0x80, 0x00)
            }

            ret :=  mload(add(memory_ptr,0x04))
        // clear memory
            mstore(0x40,memory_ptr)
        }
    }
}
