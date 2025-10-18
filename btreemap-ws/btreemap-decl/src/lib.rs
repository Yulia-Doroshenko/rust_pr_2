#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::needless_doctest_main)]

#[macro_export]
macro_rules! btreemap {
    () => {{
        ::std::collections::BTreeMap::new()
    }};
    ( $($k:expr => $v:expr),+ $(,)? ) => {{
        let mut __m = ::std::collections::BTreeMap::new();
        $( __m.insert($k, $v); )+
        __m
    }};
}
