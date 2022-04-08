<!--
 ~ Copyright (c) 2022 Espresso Systems (espressosys.com)
 ~ This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
 ~
 ~ This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
 ~ This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
 ~ You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.
 -->

# Address Book

The CAPE Address Book maps user addresses to encryption public
keys. Users publish their encryption public keys here. This is done by
submitting their UserPubKey bundle (that includes both address and
encryption public key) and signing the request using the user private
key associated with the address. This way, it is infeasible to publish
an invalid encryption key for a user.

This public mapping allows a sender to retrieve the encryption key
associated with the recipient address. This is particularly important
for the freezing feature: when the freezer releases records back to
the original owners, it needs to know their encryption keys in order
to produce owner memos for them.

The Address Book is a standalone web service.

The port defaults to 50078, but can be overridden at startup by
setting the environment variable PORT.

**POST** insert_pubkey - Insert or update the public key at the given address.

**POST** request_pubkey - Fetch the public key for the given address. If
not found, return StatusCode::NotFound.

See `tests/tests.rs` for usage examples.
