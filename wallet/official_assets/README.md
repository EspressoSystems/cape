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

The binary asset library is generated from a human readable description of the assets to include.
For the deployment of CAPE on Goerli, this specification is in `cape_v1_official_assets.toml`.
This file also references PNG icons in `icons/`. The binary asset library is generated by running
`gen-official-asset-library --assets cape_v1_official_assets.toml -o cape_v1_official_assets.lib`,
with the appropriate environment variables to connect to the Goerli deployment. The resulting
binary file is checked in as `cape_v1_official_assets.toml`, and is automatically included in the
wallet Docker image, so note that any changes to the binary libary will automatically affect the
next build of the wallet.

For the demo of CAPE on Goerli, Espresso Systems will maintain the private viewing and freezing
keys for the assets where they are applicable, but this is not theoretically required. In principle,
we could distribute the official asset library while including assets viewable or freezable only by
third parties.
