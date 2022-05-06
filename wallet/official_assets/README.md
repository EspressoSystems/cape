<!--
 ~ Copyright (c) 2022 Espresso Systems (espressosys.com)
 ~ This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
 ~
 ~ This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
 ~ This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
 ~ You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.
 -->

# The Official CAPE Asset Library

While the CAPE GUI supports arbitrary user-defined assets, a few assets will be considered
"official" and distributed along with metadata in the CAPE wallet GUI distribution. The asset
library takes the form of a binary file containing the asset definitions and metadata, with a
signature using a private signing key known only to Espresso Systems. The wallet software will check
this signature before loading any asset library file.

For the demo of CAPE on Ethereum, Espresso Systems will maintain the private viewing and freezing
keys for the assets where they are applicable, but this is not theoretically required. In principle,
we could distribute the official asset library while including assets viewable or freezable only by
third parties.

The list of asset types to be included in the first version of the official CAPE asset library
follows below. Also included in this directory are PNG icons for each asset (in `icons/`) and the
signed asset library binary (`cape_v1_official_assets.lib`). The binary was generated from a CAPE
wallet using the `gen-official-asset-library` program. The binary asset library is also included
automatically in the `wallet` Docker image, so note that any changes to the library will
automatically affect the next build of the wallet.

## Assets

### CAPEDAO ![CAPEDAO](icons/CAPEDAO.png)

- Domestic
- Not viewable
- Not freezable

### WGMI ![WGMI](icons/WGMI.png)

- Domestic
- Viewable
  - AUDPUBKEY~pqwRF_LGHH6obb4iX72qo6PWy2Fo3D9wmfqgRnYjmakB
  - The viewing key represents a hypothetical DAO admin
- Not freezable

### WGTH ![WGTH](icons/WGTH.png)

- Wraps [WETH on Goerli](https://goerli.etherscan.io/address/0xb4fbf271143f4fbf7b91a5ded31805e42b2208d6)
- Sponsored by 0xb554D84911AFa2d99f8A950cD6C5A7c9eec3c6a2
- Not viewable
- Not freezable

### DAI ![DAI](icons/DAI.png)

- Wraps [DAI](https://goerli.etherscan.io/address/0xd787ec2b6c962f611300175603741db8438674a0)
- Sponsored by 0xb554D84911AFa2d99f8A950cD6C5A7c9eec3c6a2
- Viewable
  - AUDPUBKEY~z1xFfj2B6dTdUn1tcyexZd8v8foMPDJibT8hrmXXkAhx
  - The viewing key is maintained by Espresso, but would belong to Maker Foundation in a real deployment
- Not freezable

### USDC ![USDC](icons/USDC.png)

- Wraps [USDC](https://goerli.etherscan.io/address/0x0aa78575e17ac357294bb7b5a9ea512ba07669e2)
- Sponsored by 0xb554D84911AFa2d99f8A950cD6C5A7c9eec3c6a2
- Viewable
  - AUDPUBKEY~A1EVmqt8rZc6ibPolJ0gVI-cUxTOark5L9e9TTEDt6G-
  - The viewing key is maintained by Espresso, but would belong to Circle in a real deployment
- Freezable
  - FREEZEPUBKEY~iCR07O3hlKc25lFd467Eqs3ZAF1ME1wT5aW9VqHa14e2
  - The freezing key is maintained by Espresso, but would belong to Circle in a real deployment

### USDT ![USDT](icons/USDT.png)

- Wraps [USDT](https://goerli.etherscan.io/address/0x77baa6a171e5084a9e7683b1f6658bf330bf0011)
- Sponsored by 0xb554D84911AFa2d99f8A950cD6C5A7c9eec3c6a2
- Viewable
  - AUDPUBKEY~X9pSGwzBa5nXitbGypHOnsQTA1Ddr61yDxcv4HQ0AIG5
  - The viewing key is maintained by Espresso, but would belong to Tether in a real deployment
- Freezable
  - FREEZEPUBKEY~EIvoqtawXbXQnDn2IzZNQGbMi97xZI_NLbDo6I4AKIDL
  - The freezing key is maintained by Espresso, but would belong to Tether in a real deployment
