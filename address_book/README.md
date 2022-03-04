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
