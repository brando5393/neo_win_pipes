fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/icon/windows/icon.ico");
        res.compile()
            .expect("failed to embed Windows icon resource");
    }
}
