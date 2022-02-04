//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "./libraries/BN254.sol";
import "./libraries/EdOnBN254.sol";

contract AssetRegistry {
    bytes13 public constant DOM_SEP_FOREIGN_ASSET = "FOREIGN_ASSET";

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
        // TODO check if real token (figure out if this is necessary/useful):
        //      the contract could still do whatever it wants even if it has
        //      the right interface)
        require(erc20Address != address(0), "Bad asset address");
        require(!isCapeAssetRegistered(newAsset), "Asset already registered");

        _checkForeignAssetCode(newAsset.code, erc20Address);

        bytes32 key = keccak256(abi.encode(newAsset));
        assets[key] = erc20Address;
    }

    /// @notice Checks if the asset definition code is correctly derived from the ERC20 address
    ///        of the token and the address of the depositor.
    /// @dev requires "view" to access msg.sender
    function _checkForeignAssetCode(uint256 assetDefinitionCode, address erc20Address)
        internal
        view
    {
        bytes memory description = _computeAssetDescription(erc20Address, msg.sender);
        bytes memory randomBytes = bytes.concat(
            keccak256(bytes.concat(DOM_SEP_FOREIGN_ASSET, description))
        );
        uint256 derivedCode = BN254.fromLeBytesModOrder(randomBytes);
        require(derivedCode == assetDefinitionCode, "Wrong foreign asset code");
    }

    function _computeAssetDescription(address erc20Address, address sponsor)
        internal
        pure
        returns (bytes memory)
    {
        return
            bytes.concat("TRICAPE ERC20", bytes20(erc20Address), "sponsored by", bytes20(sponsor));
    }
}
