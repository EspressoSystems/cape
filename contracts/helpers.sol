library helpers {
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
