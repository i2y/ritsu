//! SHA-256 against FIPS 180-4 (DESIGN 4.5): the published digests, and the lengths where the
//! padding takes one more block.

use ritsu_base::sha256;

#[test]
fn the_published_digests() {
    assert_eq!(sha256::hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256::hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(sha256::hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    assert_eq!(
        sha256::hex(b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu"),
        "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1"
    );
    assert_eq!(sha256::hex(&vec![b'a'; 1_000_000]), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
}

#[test]
fn the_lengths_around_a_block() {
    // 55 bytes leave room for the length in one block; 56 do not.
    for (n, want) in [
        (55, "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"),
        (56, "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"),
        (63, "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34"),
        (64, "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"),
        (65, "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0"),
        (119, "31eba51c313a5c08226adf18d4a359cfdfd8d2e816b13f4af952f7ea6584dcfb"),
        (120, "2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c"),
        (128, "6836cf13bac400e9105071cd6af47084dfacad4e5e302c94bfed24e013afb73e"),
    ] {
        assert_eq!(sha256::hex(&vec![b'a'; n]), want, "{n} bytes");
    }
    assert_eq!(sha256::hex("民法 第142条".as_bytes()), "c5b56b063a47a639356119b2c7e1f2f96ff66f796dbf82a1bafb277545bac2e2");
}

#[test]
fn the_short_form_and_the_hex_of_bytes() {
    assert_eq!(sha256::short(b"abc"), "ba7816bf8f01cfea");
    assert_eq!(sha256::to_hex(&[0x00, 0x0f, 0xab]), "000fab");
    assert_eq!(sha256::to_hex(&sha256::digest(b"abc")), sha256::hex(b"abc"));
}
