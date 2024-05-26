use bytes::{BufMut as _, BytesMut};

fn main() -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(1024);
    buf.extend_from_slice(b"hello world");
    buf.put(&b"hello world"[..]);
    //big endian
    buf.put_i64(0xdeadbeef);
    //little endian
    // buf.put_i64_le(0xdeadbeef);

    let a = buf.split();
    println!("{:?}", a);

    //get bytes from BytesMut
    let mut b = a.freeze();
    println!("{:?}", b);

    let c = b.split_to(11);
    println!("{:?}", c);
    println!("{:?}", b);

    Ok(())
}
