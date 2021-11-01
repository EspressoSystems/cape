//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract Wrapper {
    struct Policy {
        bool field; //TODO
    }

    struct AssetType {
        bool field; //TODO
    }

    struct AssetRecord {
        bool field; // TODO
    }

    function credit_erc20_balance(
        address _erc20_token,
        uint256 _amount,
        address _recipient
    ) private {}

    function gen_asset_type_erc20(
        address _erc20_token,
        string calldata _sponsor,
        Policy calldata _policy
    ) public {}

    function withdraw(
        address _recipient,
        address _erc20_token,
        uint256 _amount
    ) public {}

    function wrap_erc20_token(
        AssetType memory _asset_type,
        AssetRecord memory _record,
        address _sender
    ) public {}
}
