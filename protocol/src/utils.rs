macro_rules! impl_debug_display {
    ($struct_name:ident) => {
        impl std::fmt::Debug for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let hash: String = hex::encode(self.to_bytes());
                f.write_str(&hash)
            }
        }

        impl std::fmt::Display for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let hash: String = hex::encode(self.to_bytes());
                f.write_str(&hash)
            }
        }
    };
}

pub(crate) use impl_debug_display;

pub fn dbg_hexlify<T: AsRef<[u8]>>(
    slice: &T,
    f: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    let hexlify = hex::encode(slice);
    f.write_str(&hexlify)
}
