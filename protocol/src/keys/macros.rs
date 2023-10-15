macro_rules! impl_debug_display {
    ($struct_name:ident) => {
        impl std::fmt::Debug for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let hash: String = hex::encode(self.to_bytes());
                f.write_fmt(format_args!("{}({})", stringify!($struct_name), hash))
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
