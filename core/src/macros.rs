#[macro_export]
macro_rules! exploit {
    ($exploit:ty, $proto:expr) => {{
        #[cfg(not(feature = "no_exploits"))]
        {
            if $proto.patch {
                let mut exploit = <$exploit>::new();

                match <$exploit as Exploit<Self>>::run(&mut exploit, $proto) {
                    Ok(result) => {
                        $proto.patch = !result;
                        if let Some(patched_da) =
                            <$exploit as Exploit<Self>>::get_patched_da(&exploit)
                        {
                            $proto.da = patched_da;
                        }
                    }
                    Err(error) => {
                        log::warn!(
                            "[Exploit] {} failed: {}",
                            std::any::type_name::<$exploit>(),
                            error
                        );
                    }
                }
            }
        }
    }};
}

#[macro_export]
macro_rules! le_u16 {
    ($data:expr, $offset:expr) => {{
        assert!($data.len() >= $offset + 2, "Data length must be at least 2 bytes to read a u16");
        let bytes = &$data[$offset..$offset + 2];
        u16::from_le_bytes(bytes.try_into().expect("Failed to read u16: insufficient data"))
    }};
}

#[macro_export]
macro_rules! le_u32 {
    ($data:expr, $offset:expr) => {{
        assert!($data.len() >= $offset + 4, "Data length must be at least 4 bytes to read a u32");
        let bytes = &$data[$offset..$offset + 4];
        u32::from_le_bytes(bytes.try_into().expect("Failed to read u32: insufficient data"))
    }};
}

#[macro_export]
macro_rules! le_u64 {
    ($data:expr, $offset:expr) => {{
        assert!($data.len() >= $offset + 8, "Data length must be at least 8 bytes to read a u64");
        let bytes = &$data[$offset..$offset + 8];
        u64::from_le_bytes(bytes.try_into().expect("Failed to read u64: insufficient data"))
    }};
}
