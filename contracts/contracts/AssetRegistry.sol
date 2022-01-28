//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "./libraries/EdOnBN254.sol";

// TODO Remove once functions are implemented
/* solhint-disable no-unused-vars */

contract AssetRegistry {
    mapping(bytes32 => address) public assets;

    struct AssetDefinition {
        uint256 code;
        AssetPolicy policy;
    }

    struct AssetPolicy {
        EdOnBN254.EdOnBN254Point auditorPk;
        EdOnBN254.EdOnBN254Point credPk;
        EdOnBN254.EdOnBN254Point freezerPk;
        uint256 revealMap;
        uint64 revealThreshold;
    }

    function _lookup(AssetDefinition memory assetDefinition) internal view returns (address) {
        bytes32 key = keccak256(abi.encode(assetDefinition));
        return assets[key];
    }

    /// @notice Check if an asset is already registered
    /// @param assetDefinition describing the asset
    /// @return true if the asset type is registered, false otherwise
    function isCapeAssetRegistered(AssetDefinition memory assetDefinition)
        public
        view
        returns (bool)
    {
        return _lookup(assetDefinition) != address(0);
    }

    /// @notice Create a new asset type associated to an ERC20 token and
    ///         register it in the registry.
    /// @param erc20Address erc20 token address of corresponding to the asset type.
    /// @param newAsset asset type to be registered in the contract.
    /// @dev will revert if asset is already registered
    function sponsorCapeAsset(address erc20Address, AssetDefinition memory newAsset) public {
        // TODO check if real token (figure out if this is nececssary/useful:
        //      the contract could still do whatever it wants even if it has
        //      the right interface)
        require(erc20Address != address(0), "Bad asset address");
        require(!isCapeAssetRegistered(newAsset), "Asset already registered");
        bytes32 key = keccak256(abi.encode(newAsset));
        assets[key] = erc20Address;
    }
}
