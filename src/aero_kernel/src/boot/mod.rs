macro register_boot_protocols($($(#[$boot_meta:meta])* => $name:ident,)*) {
    $(
        $(#[$boot_meta])*
        pub mod $name;
    )*
}

register_boot_protocols!(
    #[cfg(feature = "stivale2")] => stivale2,
);
