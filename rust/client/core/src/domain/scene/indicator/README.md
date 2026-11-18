# Indicators

An indicator is one shape of its own placed in the world's doubles
(`../../view/frame/coord.rs`), tied to input or the device's state and to
nothing the world keeps: the ghost (`ghost.rs`), and the location indicator's
zone and beacon (`location.rs`). It sits and turns in the world's own space and
does not face the camera, as a marker does. The zone and a ghost box with no
cube take pxl-render `look/color.rs` `indicator`'s colors (`ACTION`, `ERASE`);
the beacon's are its own (pxl-render `param.rs` `location::beacon`).
