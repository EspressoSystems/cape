pragma solidity ^0.8.0;

contract BLAKE2_Constants {
    /*
    Constants, as defined in RFC 7693
    */

    uint64[8] public IV = [
        0x6a09e667f3bcc908,
        0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b,
        0xa54ff53a5f1d36f1,
        0x510e527fade682d1,
        0x9b05688c2b3e6c1f,
        0x1f83d9abfb41bd6b,
        0x5be0cd19137e2179
    ];

    uint64 constant MASK_0 = 0xFF00000000000000;
    uint64 constant MASK_1 = 0x00FF000000000000;
    uint64 constant MASK_2 = 0x0000FF0000000000;
    uint64 constant MASK_3 = 0x000000FF00000000;
    uint64 constant MASK_4 = 0x00000000FF000000;
    uint64 constant MASK_5 = 0x0000000000FF0000;
    uint64 constant MASK_6 = 0x000000000000FF00;
    uint64 constant MASK_7 = 0x00000000000000FF;

    uint64 constant SHIFT_0 = 0x0100000000000000;
    uint64 constant SHIFT_1 = 0x0000010000000000;
    uint64 constant SHIFT_2 = 0x0000000001000000;
    uint64 constant SHIFT_3 = 0x0000000000000100;
}
