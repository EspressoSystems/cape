// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../libraries/Utils.sol";

/// @notice Edward curve on BN254.
/// This library only implements a serialization function that is consistent with
/// Arkworks' format. It does not support any group operations.
library EdOnBN254 {
    uint256 public constant P_MOD =
        21888242871839275222246405745257275088548364400416034343698204186575808495617;

    struct EdOnBN254Point {
        uint256 x;
        uint256 y;
    }

    /// @dev check if a G1 point is Infinity
    /// @notice precompile bn256Add at address(6) takes (0, 0) as Point of Infinity,
    /// some crypto libraries (such as arkwork) uses a boolean flag to mark PoI, and
    /// just use (0, 1) as affine coordinates (not on curve) to represents PoI.
    function isInfinity(EdOnBN254Point memory point) internal pure returns (bool result) {
        assembly {
            let x := mload(point)
            let y := mload(add(point, 0x20))
            result := and(iszero(x), iszero(y))
        }
    }

    /// @dev Check if y-coordinate of G1 point is negative.
    function isYNegative(EdOnBN254Point memory point) internal pure returns (bool) {
        return (point.y << 1) < P_MOD;
    }

    function serialize(EdOnBN254Point memory point) internal pure returns (bytes memory res) {
        uint256 mask;
        // Edward curve does not have an infinity flag.
        // Set the 255-th bit to 1 for positive Y
        // See: https://github.com/arkworks-rs/algebra/blob/d6365c3a0724e5d71322fe19cbdb30f979b064c8/serialize/src/flags.rs#L148
        if (!EdOnBN254.isYNegative(point)) {
            mask = 0x8000000000000000000000000000000000000000000000000000000000000000;
        }

        return abi.encodePacked(Utils.reverseEndianness(point.x | mask));
    }
}
