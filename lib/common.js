module.exports = {

    create_chunk: function(n_aap_tx)
{
    const aap_bytes_size = 3000;

    const bytes_len = n_aap_tx * aap_bytes_size;

    const chunk = new Uint8Array(bytes_len);
    chunk.fill(12);
    return chunk;
}

}