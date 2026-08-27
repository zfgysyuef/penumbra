#[macro_export]
macro_rules! exploit {
    ($exploit:ty, $proto:expr, $port:expr, $da:expr) => {{
        #[cfg(feature = "exploits")]
        {
            if !$proto.patched {
                let mut exploit = <$exploit>::default();

                match <$exploit as $crate::exploit::Exploit<Self, P>>::run(
                    &mut exploit,
                    $proto,
                    $port,
                    $da,
                ) {
                    Ok(result) => $proto.patched = result,
                    Err(error) => log::warn!(
                        "[Exploit] {} failed: {}",
                        std::any::type_name::<$exploit>(),
                        error
                    ),
                }
            }
        }
    }};
}
