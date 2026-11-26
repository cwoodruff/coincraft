//! Build step. On Windows it embeds the app icon and product details into CoinCraft.exe,
//! so Explorer, the taskbar and the installer show them. On other systems it does nothing.

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/icon.ico");
        res.set("ProductName", "CoinCraft");
        res.set("FileDescription", "CoinCraft personal finance");
        res.set("LegalCopyright", "Copyright © 2026 Chris Woody Woodruff. MIT License.");
        if let Err(e) = res.compile() {
            // A missing resource compiler shouldn't stop the build; the app just has no icon.
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
